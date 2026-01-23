use std::collections::HashMap;
use std::sync::Arc;
use toasty::{
    db::Connect,
    driver::{Capability, Driver},
};
use toasty_core::stmt;
use tokio::sync::OnceCell;

use crate::{isolation::TestIsolation, Setup};

pub struct SetupPostgreSQL {
    isolation: TestIsolation,
    // Per-test-instance client to avoid runtime issues with static sharing
    client: OnceCell<Arc<tokio_postgres::Client>>,
}

impl SetupPostgreSQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
            client: OnceCell::new(),
        }
    }

    /// Get or create the per-test-instance PostgreSQL client
    async fn get_client(&self) -> &Arc<tokio_postgres::Client> {
        self.client
            .get_or_init(|| async {
                use tokio_postgres::NoTls;

                let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
                    .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

                let (client, connection) = tokio_postgres::connect(&url, NoTls)
                    .await
                    .unwrap_or_else(|e| panic!("PostgreSQL connection failed: {e}"));

                // Spawn the connection task
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("PostgreSQL connection error: {e}");
                    }
                });

                Arc::new(client)
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
    fn driver(&self) -> Box<dyn Driver> {
        let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
        Box::new(Connect::new(&url).unwrap())
    }

    fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let prefix = self.isolation.table_prefix();
        builder.table_name_prefix(&prefix);
    }

    fn capability(&self) -> &Capability {
        &Capability::POSTGRESQL
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        self.cleanup_postgresql_tables_impl()
            .await
            .map_err(|e| toasty::err!("PostgreSQL cleanup failed: {e}"))
    }

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, stmt::Value>,
    ) -> toasty::Result<stmt::Value> {
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
                stmt::Value::String(s) => pg_params.push(s),
                stmt::Value::I64(i) => pg_params.push(i.to_string()),
                stmt::Value::U64(u) => pg_params.push((u as i64).to_string()),
                stmt::Value::Id(id) => {
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

        // Get the per-test-instance PostgreSQL client
        let client = self.get_client().await;

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
        self.pg_row_to_stmt_value(&row, 0)
    }
}

impl SetupPostgreSQL {
    /// Cleanup PostgreSQL tables using the cached connection
    async fn cleanup_postgresql_tables_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Reuse the cached client
        let client = self.get_client().await;

        let my_prefix = self.isolation.table_prefix();
        let escaped_prefix = my_prefix
            .replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%");

        // Query for tables that belong to this test
        let rows = client
            .query(
                "SELECT table_name FROM information_schema.tables
             WHERE table_schema = 'public' AND table_name LIKE $1",
                &[&format!("{escaped_prefix}%")],
            )
            .await?;

        // Drop each table
        for row in rows {
            let table_name: String = row.get(0);
            let query = format!("DROP TABLE IF EXISTS {table_name}");
            let _ = client.simple_query(&query).await; // Ignore individual table drop errors
        }

        Ok(())
    }

    fn pg_row_to_stmt_value(
        &self,
        row: &tokio_postgres::Row,
        col: usize,
    ) -> toasty::Result<stmt::Value> {
        use tokio_postgres::types::Type;

        let column = &row.columns()[col];
        match *column.type_() {
            Type::INT2 => Ok(stmt::Value::I16(row.get(col))),
            Type::INT4 => Ok(stmt::Value::I32(row.get(col))),
            Type::INT8 => Ok(stmt::Value::I64(row.get(col))),
            Type::TEXT | Type::VARCHAR => Ok(stmt::Value::String(row.get(col))),
            Type::BYTEA => Ok(stmt::Value::Bytes(row.get(col))),
            Type::BOOL => Ok(stmt::Value::Bool(row.get(col))),
            _ => todo!("Unsupported PostgreSQL type: {:?}", column.type_()),
        }
    }
}
