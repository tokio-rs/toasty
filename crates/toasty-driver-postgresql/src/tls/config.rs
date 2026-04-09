use std::sync::Arc;

use rustls::{
    ClientConfig, DigitallySignedStruct, Error, OtherError, RootCertStore, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::CryptoProvider,
    pki_types::{CertificateDer, ServerName, UnixTime},
};

/// SSL verification mode parsed from the connection URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SslVerifyMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

pub(crate) fn build_client_config(
    mode: SslVerifyMode,
    sslrootcert: Option<&str>,
    sslcert: Option<&str>,
    sslkey: Option<&str>,
) -> Result<ClientConfig, toasty_core::Error> {
    let provider = match CryptoProvider::get_default() {
        Some(p) => p.clone(),
        None => {
            let provider = rustls::crypto::ring::default_provider();
            let _ = provider.install_default();
            CryptoProvider::get_default()
                .expect("just installed")
                .clone()
        }
    };

    let client_auth = load_client_auth(sslcert, sslkey)?;

    // sslrootcert=system -> platform verifier, enforce verify-full
    if sslrootcert == Some("system") {
        if mode != SslVerifyMode::VerifyFull {
            return Err(toasty_core::Error::invalid_connection_url(
                "sslrootcert=system requires sslmode=verify-full",
            ));
        }
        let verifier = platform_verifier(&provider)?;
        let builder = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(toasty_core::Error::driver_operation_failed)?
            .dangerous()
            .with_custom_certificate_verifier(verifier);
        return apply_client_auth(builder, client_auth);
    }

    let roots = match sslrootcert {
        Some(path) => Some(load_root_certs(path)?),
        None => None,
    };

    let verifier: Arc<dyn ServerCertVerifier> = match mode {
        SslVerifyMode::Disable => unreachable!("TLS should not be built for sslmode=disable"),

        SslVerifyMode::Prefer | SslVerifyMode::Require => {
            if let Some(roots) = roots {
                Arc::new(CaOnlyVerifier {
                    roots: Arc::new(roots),
                    provider: provider.clone(),
                })
            } else {
                Arc::new(NoVerification(provider.clone()))
            }
        }

        SslVerifyMode::VerifyCa => {
            let roots = roots.ok_or_else(|| {
                toasty_core::Error::invalid_connection_url("sslmode=verify-ca requires sslrootcert")
            })?;
            Arc::new(CaOnlyVerifier {
                roots: Arc::new(roots),
                provider: provider.clone(),
            })
        }

        SslVerifyMode::VerifyFull => {
            let roots = roots.ok_or_else(|| {
                toasty_core::Error::invalid_connection_url(
                    "sslmode=verify-full requires sslrootcert (use sslrootcert=system for OS trust store)",
                )
            })?;
            rustls::client::WebPkiServerVerifier::builder_with_provider(
                Arc::new(roots),
                provider.clone(),
            )
            .build()
            .map_err(toasty_core::Error::driver_operation_failed)?
        }
    };

    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(toasty_core::Error::driver_operation_failed)?
        .dangerous()
        .with_custom_certificate_verifier(verifier);

    apply_client_auth(builder, client_auth)
}

/// Accepts any server certificate (encryption only, no verification).
/// Used for sslmode=require/prefer without sslrootcert.
#[derive(Debug)]
struct NoVerification(Arc<CryptoProvider>);

impl ServerCertVerifier for NoVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Verifies the server certificate chain against trusted roots but does NOT
/// check that the server hostname matches the certificate.
///
/// Uses `webpki::EndEntityCert::verify_for_usage()` directly rather than
/// delegating to `WebPkiServerVerifier` and suppressing hostname errors,
/// which would rely on rustls's internal validation ordering.
///
/// Used for sslmode=verify-ca and sslmode=require with sslrootcert.
#[derive(Debug)]
struct CaOnlyVerifier {
    roots: Arc<RootCertStore>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for CaOnlyVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let cert = webpki::EndEntityCert::try_from(end_entity).map_err(pki_error)?;
        cert.verify_for_usage(
            self.provider.signature_verification_algorithms.all,
            &self.roots.roots,
            intermediates,
            now,
            webpki::KeyUsage::server_auth(),
            None,
            None,
        )
        .map_err(pki_error)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn pki_error(e: webpki::Error) -> Error {
    use rustls::CertificateError;
    use webpki::Error::*;
    match e {
        BadDer | BadDerTime | TrailingData(_) => CertificateError::BadEncoding.into(),
        CertNotValidYet { time, not_before } => {
            CertificateError::NotValidYetContext { time, not_before }.into()
        }
        CertExpired { time, not_after } => {
            CertificateError::ExpiredContext { time, not_after }.into()
        }
        InvalidCertValidity => CertificateError::Expired.into(),
        UnknownIssuer => CertificateError::UnknownIssuer.into(),
        CertRevoked => CertificateError::Revoked.into(),
        InvalidSignatureForPublicKey => CertificateError::BadSignature.into(),
        #[allow(deprecated)]
        UnsupportedSignatureAlgorithm | UnsupportedSignatureAlgorithmForPublicKey => {
            CertificateError::UnsupportedSignatureAlgorithm.into()
        }
        _ => CertificateError::Other(OtherError(Arc::new(e))).into(),
    }
}

type ClientAuthData = (
    Vec<CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
);

fn load_client_auth(
    sslcert: Option<&str>,
    sslkey: Option<&str>,
) -> Result<Option<ClientAuthData>, toasty_core::Error> {
    match (sslcert, sslkey) {
        (Some(cert_path), Some(key_path)) => {
            let cert_data =
                std::fs::read(cert_path).map_err(toasty_core::Error::driver_operation_failed)?;
            let certs: Vec<CertificateDer<'static>> =
                rustls_pemfile::certs(&mut cert_data.as_slice())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(toasty_core::Error::driver_operation_failed)?;
            if certs.is_empty() {
                return Err(toasty_core::Error::invalid_connection_url(format!(
                    "no certificates found in sslcert file: {cert_path}"
                )));
            }

            let key_data =
                std::fs::read(key_path).map_err(toasty_core::Error::driver_operation_failed)?;
            let key = rustls_pemfile::private_key(&mut key_data.as_slice())
                .map_err(toasty_core::Error::driver_operation_failed)?
                .ok_or_else(|| {
                    toasty_core::Error::invalid_connection_url(format!(
                        "no private key found in sslkey file: {key_path}"
                    ))
                })?;

            Ok(Some((certs, key)))
        }
        (None, None) => Ok(None),
        (Some(_), None) => Err(toasty_core::Error::invalid_connection_url(
            "sslcert specified without sslkey",
        )),
        (None, Some(_)) => Err(toasty_core::Error::invalid_connection_url(
            "sslkey specified without sslcert",
        )),
    }
}

fn apply_client_auth(
    builder: rustls::ConfigBuilder<ClientConfig, rustls::client::WantsClientCert>,
    client_auth: Option<ClientAuthData>,
) -> Result<ClientConfig, toasty_core::Error> {
    match client_auth {
        Some((certs, key)) => builder
            .with_client_auth_cert(certs, key)
            .map_err(toasty_core::Error::driver_operation_failed),
        None => Ok(builder.with_no_client_auth()),
    }
}

fn platform_verifier(
    provider: &Arc<CryptoProvider>,
) -> Result<Arc<dyn ServerCertVerifier>, toasty_core::Error> {
    Ok(Arc::new(
        rustls_platform_verifier::Verifier::new(provider.clone())
            .map_err(toasty_core::Error::driver_operation_failed)?,
    ))
}

fn load_root_certs(path: &str) -> Result<RootCertStore, toasty_core::Error> {
    let pem_data = std::fs::read(path).map_err(toasty_core::Error::driver_operation_failed)?;
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut pem_data.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(toasty_core::Error::driver_operation_failed)?;

    let mut store = RootCertStore::empty();
    for cert in certs {
        store
            .add(cert)
            .map_err(toasty_core::Error::driver_operation_failed)?;
    }

    if store.is_empty() {
        return Err(toasty_core::Error::invalid_connection_url(format!(
            "no certificates found in sslrootcert file: {path}"
        )));
    }

    Ok(store)
}
