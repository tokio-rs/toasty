#![cfg(feature = "mysql")]

use sqlx_core::sql_str::AssertSqlSafe;
use sqlx_mysql::{MySqlConnectOptions, MySqlPool};
use toasty_driver_mysql::MySQL;
use tokio::sync::OnceCell;

struct MySqlSetup {
    pool: OnceCell<MySqlPool>,
}

impl MySqlSetup {
    fn new() -> Self {
        Self {
            pool: OnceCell::new(),
        }
    }

    async fn get_pool(&self) -> &MySqlPool {
        self.pool
            .get_or_init(|| async {
                let url = std::env::var("TOASTY_TEST_MYSQL_URL")
                    .unwrap_or_else(|_| "mysql://toasty:toasty@localhost/toasty".to_string());
                let options = url
                    .parse::<MySqlConnectOptions>()
                    .expect("Failed to parse MySQL test URL");
                MySqlPool::connect_with(options)
                    .await
                    .expect("Failed to connect to MySQL")
            })
            .await
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for MySqlSetup {
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
        let url = std::env::var("TOASTY_TEST_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://toasty:toasty@localhost/toasty".to_string());
        Box::new(MySQL::new(url.as_str()).expect("Failed to create MySQL driver"))
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

// Generate all driver tests
toasty_driver_integration_suite::generate_driver_tests!(MySqlSetup::new(),
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
    document_collections: true,
    vec_remove: false,
    vec_pop: false,
    vec_remove_at: false,
    transaction_lock_mode: false,
);
