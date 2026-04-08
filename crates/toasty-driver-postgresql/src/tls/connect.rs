// Vendored from tokio-postgres-rustls
// https://github.com/jbg/tokio-postgres-rustls/commit/4326f72863ff8f205a71773a5f8b8467e8cd699a
//
// MIT License
//
// Copyright (c) 2019 Jasper Hugo
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//
// Changes from upstream:
// - Replaced ring::digest with sha2::{Sha256, Sha384, Sha512}
// - Removed test module
// - Removed module-level doc include
// - Adjusted lint attributes to match workspace conventions

use std::{convert::TryFrom, sync::Arc};

use rustls::{ClientConfig, pki_types::ServerName};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_postgres::tls::MakeTlsConnect;

use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use const_oid::db::{
    rfc5912::{
        ECDSA_WITH_SHA_256, ECDSA_WITH_SHA_384, ID_SHA_1, ID_SHA_256, ID_SHA_384, ID_SHA_512,
        SHA_1_WITH_RSA_ENCRYPTION, SHA_256_WITH_RSA_ENCRYPTION, SHA_384_WITH_RSA_ENCRYPTION,
        SHA_512_WITH_RSA_ENCRYPTION,
    },
    rfc8410::ID_ED_25519,
};
use sha2::{Digest, Sha256, Sha384, Sha512};
use tokio::io::ReadBuf;
use tokio_postgres::tls::{ChannelBinding, TlsConnect};
use tokio_rustls::{TlsConnector, client::TlsStream};
use x509_cert::{Certificate, der::Decode};

pub(crate) struct TlsConnectFuture<S> {
    inner: tokio_rustls::Connect<S>,
}

impl<S> Future for TlsConnectFuture<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    type Output = io::Result<RustlsStream<S>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx).map_ok(RustlsStream)
    }
}

pub(crate) struct RustlsConnect(RustlsConnectData);

pub(crate) struct RustlsConnectData {
    hostname: ServerName<'static>,
    connector: TlsConnector,
}

impl<S> TlsConnect<S> for RustlsConnect
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Stream = RustlsStream<S>;
    type Error = io::Error;
    type Future = TlsConnectFuture<S>;

    fn connect(self, stream: S) -> Self::Future {
        TlsConnectFuture {
            inner: self.0.connector.connect(self.0.hostname, stream),
        }
    }
}

pub(crate) struct RustlsStream<S>(TlsStream<S>);

impl<S> tokio_postgres::tls::TlsStream for RustlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn channel_binding(&self) -> ChannelBinding {
        let (_, session) = self.0.get_ref();
        match session.peer_certificates() {
            Some(certs) if !certs.is_empty() => Certificate::from_der(&certs[0])
                .ok()
                .and_then(|cert| {
                    let hash: Vec<u8> = match cert.signature_algorithm.oid {
                        // SHA1 is upgraded to SHA256 per https://datatracker.ietf.org/doc/html/rfc5929#section-4.1
                        ID_SHA_1
                        | ID_SHA_256
                        | SHA_1_WITH_RSA_ENCRYPTION
                        | SHA_256_WITH_RSA_ENCRYPTION
                        | ECDSA_WITH_SHA_256 => Sha256::digest(certs[0].as_ref()).to_vec(),
                        ID_SHA_384 | SHA_384_WITH_RSA_ENCRYPTION | ECDSA_WITH_SHA_384 => {
                            Sha384::digest(certs[0].as_ref()).to_vec()
                        }
                        ID_SHA_512 | SHA_512_WITH_RSA_ENCRYPTION | ID_ED_25519 => {
                            Sha512::digest(certs[0].as_ref()).to_vec()
                        }
                        _ => return None,
                    };
                    Some(hash)
                })
                .map_or_else(ChannelBinding::none, |hash| {
                    ChannelBinding::tls_server_end_point(hash)
                }),
            _ => ChannelBinding::none(),
        }
    }
}

impl<S> AsyncRead for RustlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<tokio::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for RustlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<tokio::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<tokio::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

/// A `MakeTlsConnect` implementation using `rustls`.
#[derive(Clone, Debug)]
pub(crate) struct MakeRustlsConnect {
    config: Arc<ClientConfig>,
}

impl MakeRustlsConnect {
    pub(crate) fn new(config: ClientConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> MakeTlsConnect<S> for MakeRustlsConnect
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Stream = RustlsStream<S>;
    type TlsConnect = RustlsConnect;
    type Error = rustls::pki_types::InvalidDnsNameError;

    fn make_tls_connect(&mut self, hostname: &str) -> Result<Self::TlsConnect, Self::Error> {
        ServerName::try_from(hostname).map(|dns_name| {
            RustlsConnect(RustlsConnectData {
                hostname: dns_name.to_owned(),
                connector: Arc::clone(&self.config).into(),
            })
        })
    }
}
