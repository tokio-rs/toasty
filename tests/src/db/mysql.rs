use toasty::driver::Capability;
use toasty::{db, Db};

use crate::{isolation::TestIsolation, Setup};

pub struct SetupMySQL {
    isolation: TestIsolation,
}

impl SetupMySQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
        }
    }
}

impl Default for SetupMySQL {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupMySQL {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        let prefix = self.isolation.table_prefix();

        let url = std::env::var("TOASTY_TEST_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());

        builder.table_name_prefix(&prefix).connect(&url).await
    }

    fn capability(&self) -> &Capability {
        &Capability::MYSQL
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        cleanup_mysql_tables(&self.isolation)
            .await
            .map_err(|e| toasty::Error::msg(format!("MySQL cleanup failed: {}", e)))
    }
}

async fn cleanup_mysql_tables(isolation: &TestIsolation) -> Result<(), Box<dyn std::error::Error>> {
    use mysql_async::prelude::*;

    let url = std::env::var("TOASTY_TEST_MYSQL_URL")
        .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());

    let opts = mysql_async::Opts::from_url(&url)?;
    let pool = mysql_async::Pool::new(opts);
    let mut conn = pool.get_conn().await?;

    let my_prefix = isolation.table_prefix();

    // Query for tables that belong to this test
    let rows: Vec<String> = conn
        .query(format!(
            "SELECT table_name FROM information_schema.tables
         WHERE table_schema = DATABASE() AND table_name LIKE '{}%'",
            my_prefix
        ))
        .await?;

    // Drop each table
    for table_name in rows {
        let query = format!("DROP TABLE IF EXISTS {}", table_name);
        let _ = conn.query_drop(&query).await; // Ignore individual table drop errors
    }

    drop(conn);
    pool.disconnect().await?;
    Ok(())
}
