#![cfg(feature = "sqlite")]

use std::time::Duration;

use toasty::{Db, stmt::Value};
use toasty_driver_sqlite::{JournalMode, Sqlite, Synchronous};

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,

    name: String,
}

async fn build(driver: Sqlite) -> toasty::Result<Db> {
    Db::builder()
        .models(toasty::models!(User))
        .build(driver)
        .await
}

/// Reads a pragma back over a pooled connection, which is only configured if
/// the driver applied pragmas on connect.
async fn read(db: &mut Db, pragma: &str) -> String {
    let rows = toasty::sql::query(format!("PRAGMA {pragma}"))
        .exec(db)
        .await
        .unwrap_or_else(|err| panic!("reading `{pragma}` should succeed: {err}"));

    match &rows[0] {
        Value::Record(record) => match &record[0] {
            Value::String(value) => value.clone(),
            Value::I64(value) => value.to_string(),
            other => panic!("unexpected `{pragma}` value: {other:?}"),
        },
        other => panic!("unexpected `{pragma}` row: {other:?}"),
    }
}

#[tokio::test]
async fn foreign_keys_and_busy_timeout_are_set_by_default() {
    let mut db = build(Sqlite::in_memory()).await.unwrap();

    assert_eq!(read(&mut db, "foreign_keys").await, "1");
    assert_eq!(read(&mut db, "busy_timeout").await, "5000");
}

#[tokio::test]
async fn foreign_keys_can_be_disabled() {
    let mut db = build(Sqlite::in_memory().foreign_keys(false))
        .await
        .unwrap();

    assert_eq!(read(&mut db, "foreign_keys").await, "0");
}

#[tokio::test]
async fn busy_timeout_applies_to_pooled_connections() {
    let driver = Sqlite::in_memory().busy_timeout(Duration::from_millis(1_234));
    let mut db = build(driver).await.unwrap();

    assert_eq!(read(&mut db, "busy_timeout").await, "1234");
}

#[tokio::test]
async fn typed_methods_apply_to_a_file_database() {
    let dir = tempfile::tempdir().unwrap();
    let driver = Sqlite::open(dir.path().join("app.db"))
        .journal_mode(JournalMode::Wal)
        .synchronous(Synchronous::Normal);
    let mut db = build(driver).await.unwrap();

    assert_eq!(read(&mut db, "journal_mode").await, "wal");
    // NORMAL is 1.
    assert_eq!(read(&mut db, "synchronous").await, "1");
}

#[tokio::test]
async fn pragma_sets_a_pragma_without_a_method() {
    let mut db = build(Sqlite::in_memory().pragma("temp_store", "MEMORY"))
        .await
        .unwrap();

    // MEMORY is 2.
    assert_eq!(read(&mut db, "temp_store").await, "2");
}

#[tokio::test]
async fn pragma_and_typed_methods_share_a_setting() {
    let dir = tempfile::tempdir().unwrap();
    let driver = Sqlite::open(dir.path().join("app.db"))
        .journal_mode(JournalMode::Delete)
        .pragma("journal_mode", "WAL");
    let mut db = build(driver).await.unwrap();

    assert_eq!(read(&mut db, "journal_mode").await, "wal");
}

#[tokio::test]
async fn a_pragma_sqlite_rejects_fails_the_connection() {
    build(Sqlite::in_memory().pragma("cache_size", "not a value"))
        .await
        .expect_err("an invalid pragma value should fail the connection");
}

#[test]
fn debug_output_withholds_secret_pragma_values() {
    let driver = Sqlite::in_memory()
        .pragma("key", "'a passphrase'")
        .journal_mode(JournalMode::Wal);

    let debug = format!("{driver:?}");
    assert!(
        !debug.contains("a passphrase"),
        "passphrase leaked: {debug}"
    );
    assert!(debug.contains("[redacted]"), "unexpected debug: {debug}");
    // Non-secret pragmas stay legible.
    assert!(debug.contains("WAL"), "unexpected debug: {debug}");
}

#[cfg(feature = "sqlite-sqlcipher")]
#[tokio::test]
async fn an_encrypted_database_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    // The quote checks that `key` escapes the passphrase.
    let keyed = || Sqlite::open(&path).key("it's a passphrase");

    let mut db = build(keyed()).await.unwrap();
    db.push_schema().await.unwrap();
    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();
    drop(db);

    // Without the right key, connecting runs nothing that reads the file, so
    // the failure surfaces at the first query.
    for driver in [Sqlite::open(&path), Sqlite::open(&path).key("wrong")] {
        let mut db = build(driver).await.unwrap();
        User::get_by_id(&mut db, &1)
            .await
            .expect_err("an encrypted database should not be readable without its key");
    }

    let mut db = build(keyed()).await.unwrap();
    let user = User::get_by_id(&mut db, &1).await.unwrap();
    assert_eq!(user.name, "Alice");
}

#[cfg(feature = "sqlite-sqlcipher")]
#[tokio::test]
async fn a_failing_secret_pragma_does_not_leak_its_value() {
    // An unterminated literal is a syntax error, and SQLite's message would
    // otherwise quote the statement.
    let err = build(Sqlite::in_memory().pragma("key", "'a passphrase"))
        .await
        .expect_err("a malformed key should fail the connection")
        .to_string();

    assert!(!err.contains("a passphrase"), "passphrase leaked: {err}");
}
