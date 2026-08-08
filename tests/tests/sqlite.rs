#![cfg(feature = "sqlite")]

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
    native_ilike: false,
    native_json: false,
    native_jsonb: false,
    native_array: false,
    native_enum: false,
    vec_scalar: true,
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
