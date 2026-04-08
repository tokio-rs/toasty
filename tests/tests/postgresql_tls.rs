#![cfg(feature = "postgresql")]

use toasty_driver_postgresql::PostgreSQL;

fn tls_url() -> String {
    std::env::var("TOASTY_TEST_POSTGRES_TLS_URL")
        .unwrap_or_else(|_| "postgresql://toasty:toasty@localhost:5433/toasty".to_string())
}

fn certs_dir() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tls/certs")
}

fn ca_cert_path() -> String {
    format!("{}/ca.crt", certs_dir())
}

async fn smoke_query(driver: &PostgreSQL) {
    use toasty_core::driver::Driver;
    let conn = driver.connect().await.expect("connection failed");
    drop(conn);
}

#[tokio::test]
async fn tls_require() {
    let url = format!("{}?sslmode=require", tls_url());
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn tls_prefer() {
    let url = format!("{}?sslmode=prefer", tls_url());
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn tls_channel_binding() {
    let url = format!("{}?sslmode=require&channel_binding=require", tls_url());
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn tls_disable_against_tls_server() {
    let url = format!("{}?sslmode=disable", tls_url());
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    use toasty_core::driver::Driver;
    let result = driver.connect().await;
    assert!(
        result.is_err(),
        "expected connection to fail with sslmode=disable against TLS-only server"
    );
}

#[tokio::test]
async fn sslrootcert_require() {
    let url = format!(
        "{}?sslmode=require&sslrootcert={}",
        tls_url(),
        ca_cert_path()
    );
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn sslrootcert_wrong_ca() {
    use toasty_core::driver::Driver;

    let wrong_ca = format!("{}/client.crt", certs_dir());

    let url = format!("{}?sslmode=require&sslrootcert={}", tls_url(), wrong_ca);
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    let result = driver.connect().await;
    assert!(
        result.is_err(),
        "expected connection to fail with wrong CA certificate"
    );
}

#[tokio::test]
async fn verify_ca() {
    let url = format!(
        "{}?sslmode=verify-ca&sslrootcert={}",
        tls_url(),
        ca_cert_path()
    );
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn verify_full() {
    let url = format!(
        "{}?sslmode=verify-full&sslrootcert={}",
        tls_url(),
        ca_cert_path()
    );
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn verify_full_hostname_mismatch() {
    use toasty_core::driver::Driver;

    // Connect via 127.0.0.1 instead of localhost. The cert SAN has
    // DNS:localhost,IP:127.0.0.1 -- but tokio-postgres resolves the host to
    // an IP and uses it for TLS. We override the host to force a mismatch
    // by using a hostname not in the cert.
    let base = tls_url();
    let url = base.replace("localhost", "127.0.0.2");
    let url = format!("{}?sslmode=verify-full&sslrootcert={}", url, ca_cert_path());

    match PostgreSQL::new(&url) {
        Ok(driver) => {
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(5), driver.connect()).await;
            match result {
                Ok(Ok(_)) => panic!("expected connection to fail with hostname mismatch"),
                Ok(Err(_)) | Err(_) => {} // TLS error or timeout, both acceptable
            }
        }
        Err(_) => {} // acceptable: hostname resolution failure
    }
}

#[tokio::test]
async fn require_without_sslrootcert() {
    let url = format!("{}?sslmode=require", tls_url());
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}
