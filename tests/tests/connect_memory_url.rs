#[cfg(feature = "sqlite")]
#[tokio::test]
async fn connect_sqlite_in_memory_urls() {
    for url in ["sqlite::memory:", "sqlite://:memory:", "SQLITE://:memory:"] {
        toasty::Db::builder()
            .connect(url)
            .await
            .unwrap_or_else(|err| panic!("connecting with `{url}` should succeed: {err}"));
    }
}
