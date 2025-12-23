#![cfg(feature = "mysql")]

use mysql_async::prelude::Queryable;
use tokio::sync::OnceCell;

struct MySqlSetup {
    pool: OnceCell<mysql_async::Pool>,
}

impl MySqlSetup {
    fn new() -> Self {
        Self {
            pool: OnceCell::new(),
        }
    }

    async fn get_pool(&self) -> &mysql_async::Pool {
        self.pool
            .get_or_init(|| async {
                let url = std::env::var("TOASTY_TEST_MYSQL_URL")
                    .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());
                mysql_async::Pool::new(url.as_str())
            })
            .await
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for MySqlSetup {
    fn driver(&self) -> Box<dyn toasty::driver::Driver> {
        let url = std::env::var("TOASTY_TEST_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());
        Box::new(toasty::db::Connect::new(&url).expect("Failed to create MySQL driver"))
    }

    async fn delete_table(&self, name: &str) {
        let pool = self.get_pool().await;
        let mut conn = pool.get_conn().await.expect("Failed to get connection");

        let sql = format!("DROP TABLE IF EXISTS `{}`", name);
        conn.query_drop(&sql).await.expect("Failed to drop table");
    }
}

// Generate all driver tests
toasty_driver_integration_suite::generate_driver_tests!(MySqlSetup::new());
