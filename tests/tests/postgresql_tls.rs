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

    // test.localtest.me resolves to 127.0.0.1 but is not in the certificate
    // SAN (DNS:localhost,IP:127.0.0.1), so verify-full should reject it.
    let base = tls_url();
    let url = base.replace("localhost", "test.localtest.me");
    let url = format!("{}?sslmode=verify-full&sslrootcert={}", url, ca_cert_path());

    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    let result = driver.connect().await;
    assert!(
        result.is_err(),
        "expected connection to fail with hostname mismatch"
    );
}

#[tokio::test]
async fn verify_ca_hostname_mismatch() {
    // test.localtest.me resolves to 127.0.0.1 but is not in the certificate
    // SAN (DNS:localhost,IP:127.0.0.1). verify-ca should still accept this
    // because it does not check the hostname.
    let base = tls_url();
    let url = base.replace("localhost", "test.localtest.me");
    let url = format!("{}?sslmode=verify-ca&sslrootcert={}", url, ca_cert_path());

    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn verify_ca_wrong_ca() {
    use toasty_core::driver::Driver;

    let wrong_ca = format!("{}/client.crt", certs_dir());
    let url = format!("{}?sslmode=verify-ca&sslrootcert={}", tls_url(), wrong_ca);
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    let result = driver.connect().await;
    assert!(
        result.is_err(),
        "expected verify-ca to reject certificate signed by untrusted CA"
    );
}

#[test]
fn verify_ca_requires_sslrootcert() {
    let url = format!("{}?sslmode=verify-ca", tls_url());
    let result = PostgreSQL::new(&url);
    assert!(
        result.is_err(),
        "expected error when verify-ca used without sslrootcert"
    );
}

#[test]
fn verify_full_requires_sslrootcert() {
    let url = format!("{}?sslmode=verify-full", tls_url());
    let result = PostgreSQL::new(&url);
    assert!(
        result.is_err(),
        "expected error when verify-full used without sslrootcert"
    );
}

#[tokio::test]
async fn require_without_sslrootcert() {
    let url = format!("{}?sslmode=require", tls_url());
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[tokio::test]
async fn client_cert_auth() {
    let dir = certs_dir();
    let url = format!(
        "{}?sslmode=verify-full&sslrootcert={}/ca.crt&sslcert={}/client.crt&sslkey={}/client.key",
        tls_url(),
        dir,
        dir,
        dir
    );
    let driver = PostgreSQL::new(&url).expect("driver creation failed");
    smoke_query(&driver).await;
}

#[test]
fn missing_sslkey() {
    let dir = certs_dir();
    let url = format!("{}?sslcert={}/client.crt", tls_url(), dir);
    let result = PostgreSQL::new(&url);
    assert!(
        result.is_err(),
        "expected error when sslcert set without sslkey"
    );
}

#[test]
fn missing_sslcert() {
    let dir = certs_dir();
    let url = format!("{}?sslkey={}/client.key", tls_url(), dir);
    let result = PostgreSQL::new(&url);
    assert!(
        result.is_err(),
        "expected error when sslkey set without sslcert"
    );
}
