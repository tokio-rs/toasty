#[cfg(feature = "sqlite")]
#[tokio::test]
async fn connect_bad_url_sqlite() {
    let result = toasty::Db::builder()
        .connect("sqlite:///nonexistent_dir_xyz/db.sqlite")
        .await;
    assert!(
        result.unwrap_err().is_connection_pool(),
        "connecting with a bad SQLite URL should fail"
    );
}

#[cfg(feature = "postgresql")]
#[tokio::test]
async fn connect_bad_url_postgresql() {
    let result = toasty::Db::builder()
        .connect("postgresql://localhost:1/bad")
        .await;
    assert!(
        result.unwrap_err().is_connection_pool(),
        "connecting with a bad PostgreSQL URL should fail"
    );
}

#[cfg(feature = "mysql")]
#[tokio::test]
async fn connect_bad_url_mysql() {
    let result = toasty::Db::builder()
        .connect("mysql://localhost:1/bad")
        .await;
    assert!(
        result.unwrap_err().is_connection_pool(),
        "connecting with a bad MySQL URL should fail"
    );
}
