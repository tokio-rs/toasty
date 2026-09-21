#![cfg(feature = "mariadb")]

//! Driver integration suite against MariaDB.
//!
//! `TOASTY_TEST_MARIADB_URL` must point at MariaDB 11.8 or later.

use sqlx_core::sql_str::AssertSqlSafe;
use sqlx_mysql::{MySqlConnectOptions, MySqlPool};
use toasty_driver_mysql::MariaDB;
use tokio::sync::OnceCell;

fn url() -> String {
    std::env::var("TOASTY_TEST_MARIADB_URL")
        .unwrap_or_else(|_| "mariadb://toasty:toasty@localhost:3307/toasty".to_string())
}

struct MariaDbSetup {
    pool: OnceCell<MySqlPool>,
}

impl MariaDbSetup {
    fn new() -> Self {
        Self {
            pool: OnceCell::new(),
        }
    }

    async fn get_pool(&self) -> &MySqlPool {
        self.pool
            .get_or_init(|| async {
                let options = url()
                    .parse::<MySqlConnectOptions>()
                    .expect("Failed to parse MariaDB test URL");
                MySqlPool::connect_with(options)
                    .await
                    .expect("Failed to connect to MariaDB")
            })
            .await
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for MariaDbSetup {
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
        Box::new(MariaDB::new(url()).expect("Failed to create MariaDB driver"))
    }

    async fn delete_table(&self, name: &str) {
        let pool = self.get_pool().await;
        let sql = format!("DROP TABLE IF EXISTS `{}`", name);
        sqlx_core::query::query(AssertSqlSafe(sql))
            .execute(pool)
            .await
            .expect("Failed to drop table");
    }
}

// Flags match `mysql.rs` except `returning_from_insert`.
toasty_driver_integration_suite::generate_driver_tests!(MariaDbSetup::new(),
    cte_unreferenced: false,
    decimal_arbitrary_precision: false,
    native_ilike: false,
    native_jsonb: false,
    native_cidr: false,
    native_inet: false,
    native_macaddr: false,
    native_macaddr8: false,
    upsert_primary_key: false,
    upsert_unique: false,
    upsert_branch_assignments: false,
    upsert_targeted_ignore: false,
    native_array: false,
    vec_scalar: true,
    unique_list_index: false,
    document_collections: true,
    returning_from_insert: true,
    vec_remove: false,
    vec_pop: false,
    vec_remove_at: false,
    transaction_lock_mode: false,
);
