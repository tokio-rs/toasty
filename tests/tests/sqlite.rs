#![cfg(feature = "sqlite")]

use toasty_driver_sqlite::{JournalMode, Sqlite};

struct SqliteSetup;

impl SqliteSetup {
    fn new() -> Self {
        SqliteSetup
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for SqliteSetup {
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
        Box::new(toasty_driver_sqlite::Sqlite::in_memory())
    }

    async fn delete_table(&self, _name: &str) {
        // There is no need to delete anything since the driver operates in-memory
    }
}

// Generate all driver tests
toasty_driver_integration_suite::generate_driver_tests!(
    SqliteSetup::new(),
    native_decimal: false,
    bigdecimal_implemented: false,
    decimal_arbitrary_precision: false,
    native_timestamp: false,
    native_date: false,
    native_time: false,
    native_datetime: false,
    native_cidr: false,
    native_inet: false,
    native_macaddr: false,
    native_macaddr8: false,
    native_ilike: false,
    native_json: false,
    native_jsonb: false,
    native_array: false,
    native_enum: false,
    vec_scalar: true,
    unique_list_index: false,
    document_collections: true,
    vec_remove: false,
    vec_pop: false,
    vec_remove_at: false,
    test_connection_pool: false,
);

#[derive(Debug, toasty::Model)]
struct PoolItem {
    #[key]
    #[auto]
    id: i64,
}

/// In-memory SQLite forces `max_connections = 1`; verify the driver cap
/// overrides a larger user-requested pool size.
#[tokio::test]
async fn in_memory_caps_user_max_pool_size() {
    let db = toasty::Db::builder()
        .models(toasty::models!(PoolItem))
        .max_pool_size(16)
        .build(toasty_driver_sqlite::Sqlite::in_memory())
        .await
        .unwrap();

    assert_eq!(db.pool().status().max_size, 1);
}

#[tokio::test]
async fn auto_i64_key_uses_sqlite_integer_storage_type() {
    let db = toasty::Db::builder()
        .models(toasty::models!(PoolItem))
        .build(toasty_driver_sqlite::Sqlite::in_memory())
        .await
        .unwrap();

    let id = &db.schema().db.tables[0].columns[0];

    assert_eq!(id.storage_ty, toasty::schema::db::Type::Integer(4));
    assert!(id.auto_increment);
}

#[derive(Debug, toasty::Model)]
struct ResetItem {
    #[key]
    #[auto]
    id: i64,

    name: String,
}

async fn build_file_db(driver: Sqlite) -> toasty::Db {
    toasty::Db::builder()
        .models(toasty::models!(ResetItem))
        .build(driver)
        .await
        .unwrap()
}

/// Deleting the database file fails on Windows while the pool holds an idle
/// connection open (`ERROR_SHARING_VIOLATION`), and on POSIX the pooled
/// connections would keep writing to the unlinked file. `reset_db` clears the
/// schema in place instead.
#[tokio::test]
async fn reset_db_succeeds_with_an_idle_pooled_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reset.db");

    let mut db = build_file_db(Sqlite::open(&path)).await;
    db.push_schema().await.unwrap();
    toasty::create!(ResetItem { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();

    // The pool keeps a connection open to the file, which is what made the
    // old `remove_file` implementation fail on Windows.
    assert!(db.pool().status().available >= 1);

    db.reset_db().await.unwrap();

    // The same handle still works, but the tables are gone.
    ResetItem::all()
        .exec(&mut db)
        .await
        .expect_err("reset_db should drop every table");
    assert!(path.exists(), "reset_db must not unlink the database file");

    // The pool is usable again, and AUTOINCREMENT starts over.
    db.push_schema().await.unwrap();
    let item = toasty::create!(ResetItem { name: "Bob" })
        .exec(&mut db)
        .await
        .unwrap();
    assert_eq!(item.id, 1);
}

/// `reset_db` also has to work when the database is in WAL mode, where the
/// `-wal` sidecar holds not-yet-checkpointed pages.
#[tokio::test]
async fn reset_db_works_in_wal_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wal.db");

    let mut db = build_file_db(Sqlite::open(&path).journal_mode(JournalMode::Wal)).await;
    db.push_schema().await.unwrap();
    toasty::create!(ResetItem { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();

    db.reset_db().await.unwrap();

    ResetItem::all()
        .exec(&mut db)
        .await
        .expect_err("reset_db should drop every table");
}
