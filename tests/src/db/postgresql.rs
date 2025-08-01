use toasty::driver::Capability;
use toasty::{db, Db};

use crate::{isolation::TestIsolation, Setup};

pub struct SetupPostgreSQL {
    isolation: TestIsolation,
}

impl SetupPostgreSQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
        }
    }
}

impl Default for SetupPostgreSQL {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupPostgreSQL {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        let prefix = self.isolation.table_prefix();

        let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

        builder.table_name_prefix(&prefix).connect(&url).await
    }

    fn capability(&self) -> &Capability {
        &Capability::POSTGRESQL
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        cleanup_postgresql_tables(&self.isolation)
            .await
            .map_err(|e| toasty::Error::msg(format!("PostgreSQL cleanup failed: {}", e)))
    }
}

async fn cleanup_postgresql_tables(
    isolation: &TestIsolation,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio_postgres::NoTls;

    let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

    let (client, connection) = tokio_postgres::connect(&url, NoTls).await?;

    // Spawn the connection task
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error during cleanup: {}", e);
        }
    });

    let my_prefix = isolation.table_prefix();

    // Query for tables that belong to this test
    let rows = client
        .query(
            "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name LIKE $1",
            &[&format!("{}%", my_prefix)],
        )
        .await?;

    // Drop each table
    for row in rows {
        let table_name: String = row.get(0);
        let query = format!("DROP TABLE IF EXISTS {} CASCADE", table_name);
        let _ = client.execute(&query, &[]).await; // Ignore individual table drop errors
    }

    Ok(())
}
