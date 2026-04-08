#![cfg(feature = "postgresql")]

use toasty_driver_postgresql::PostgreSQL;

fn tls_url() -> String {
    std::env::var("TOASTY_TEST_POSTGRES_TLS_URL")
        .unwrap_or_else(|_| "postgresql://toasty:toasty@localhost:5433/toasty".to_string())
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
