#![cfg(feature = "sqlite")]

use std::time::Duration;

use toasty::models;
use toasty_driver_sqlite::Sqlite;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: i64,
}

#[tokio::test]
async fn driver_max_connections_caps_user_request() {
    // In-memory SQLite forces max_connections = 1; it must override a
    // larger user-requested value.
    let db = toasty::Db::builder()
        .models(models!(Item))
        .max_pool_size(16)
        .build(Sqlite::in_memory())
        .await
        .unwrap();

    assert_eq!(db.pool().status().max_size, 1);
}

#[tokio::test]
async fn pool_timeouts_are_accepted() {
    // We cannot easily observe the timeout values through the public API,
    // but the builder should accept them without error.
    toasty::Db::builder()
        .models(models!(Item))
        .pool_wait_timeout(Some(Duration::from_millis(500)))
        .pool_create_timeout(Some(Duration::from_secs(2)))
        .build(Sqlite::in_memory())
        .await
        .unwrap();
}
