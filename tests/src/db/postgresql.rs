use std::collections::HashMap;
use toasty::driver::Capability;
use toasty::{db, Db};

use crate::{isolation::TestIsolation, Setup};

// Global lazy PostgreSQL client to avoid creating connections on each call
static POSTGRESQL_CLIENT: tokio::sync::OnceCell<tokio_postgres::Client> =
    tokio::sync::OnceCell::const_new();

pub struct SetupPostgreSQL {
    isolation: TestIsolation,
}

impl SetupPostgreSQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
        }
    }

    /// Get or create the global PostgreSQL client
    async fn get_client() -> &'static tokio_postgres::Client {
        POSTGRESQL_CLIENT
            .get_or_init(|| async {
                use tokio_postgres::NoTls;

                let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
                    .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
                let (client, connection) = tokio_postgres::connect(&url, NoTls)
                    .await
                    .unwrap_or_else(|e| panic!("PostgreSQL connection failed: {e}"));

                // Spawn the connection task
                tokio::spawn(async move {
                    let _ = connection.await;
                });

                client
            })
            .await
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
            .map_err(|e| toasty::Error::msg(format!("PostgreSQL cleanup failed: {e}")))
    }

    async fn get_raw_column_value<T>(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: TryFrom<toasty_core::stmt::Value, Error = toasty_core::Error>,
    {
        let full_table_name = format!("{}{}", self.isolation.table_prefix(), table);

        // Build WHERE clause from filter
        let mut where_conditions = Vec::new();
        let mut param_index = 1;

        // Convert stmt::Values to PostgreSQL parameters
        let mut pg_params = Vec::new();
        for (col_name, value) in filter {
            where_conditions.push(format!("{col_name} = ${param_index}"));

            // Convert each value individually to avoid trait bound issues
            match value {
                toasty_core::stmt::Value::String(s) => pg_params.push(s),
                toasty_core::stmt::Value::I64(i) => pg_params.push(i.to_string()),
                toasty_core::stmt::Value::U64(u) => pg_params.push((u as i64).to_string()),
                toasty_core::stmt::Value::Id(id) => {
                    // Convert Id to string representation
                    pg_params.push(id.to_string());
                }
                _ => todo!("Unsupported filter value type: {value:?}"),
            }
            param_index += 1;
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        let query = format!("SELECT {column} FROM {full_table_name}{where_clause}");

        // Get the cached PostgreSQL client
        let client = Self::get_client().await;

        // For simplicity, use string parameters for now
        let string_params: Vec<&str> = pg_params.iter().map(|s| s.as_str()).collect();
        let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = string_params
            .iter()
            .map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();
        let row = client
            .query_one(&query, &params_refs)
            .await
            .unwrap_or_else(|e| panic!("Query failed: {e}"));

        // Convert PostgreSQL result directly to stmt::Value
        let stmt_value = self.pg_row_to_stmt_value(&row, 0)?;

        // Let the type implementation validate and convert
        stmt_value
            .try_into()
            .map_err(|e: toasty_core::Error| panic!("Validation failed: {e}"))
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
            eprintln!("PostgreSQL connection error during cleanup: {e}");
        }
    });

    let my_prefix = isolation.table_prefix();

    // Query for tables that belong to this test
    let rows = client
        .query(
            "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name LIKE $1",
            &[&format!("{my_prefix}%")],
        )
        .await?;

    // Drop each table
    for row in rows {
        let table_name: String = row.get(0);
        let query = format!("DROP TABLE IF EXISTS {table_name} CASCADE");
        let _ = client.execute(&query, &[]).await; // Ignore individual table drop errors
    }

    Ok(())
}

impl SetupPostgreSQL {
    fn pg_row_to_stmt_value(
        &self,
        row: &tokio_postgres::Row,
        col: usize,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use tokio_postgres::types::Type;

        let column = &row.columns()[col];
        match *column.type_() {
            Type::INT2 => Ok(toasty_core::stmt::Value::I16(row.get(col))),
            Type::INT4 => Ok(toasty_core::stmt::Value::I32(row.get(col))),
            Type::INT8 => Ok(toasty_core::stmt::Value::I64(row.get(col))),
            Type::TEXT | Type::VARCHAR => Ok(toasty_core::stmt::Value::String(row.get(col))),
            Type::BOOL => Ok(toasty_core::stmt::Value::Bool(row.get(col))),
            _ => todo!("Unsupported PostgreSQL type: {:?}", column.type_()),
        }
    }
}
