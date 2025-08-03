use std::collections::HashMap;
use toasty::driver::Capability;
use toasty::{db, Db};

use crate::{isolation::TestIsolation, Setup};

// Global lazy MySQL connection pool to avoid creating connections on each call
static MYSQL_POOL: tokio::sync::OnceCell<mysql_async::Pool> = tokio::sync::OnceCell::const_new();

pub struct SetupMySQL {
    isolation: TestIsolation,
}

impl SetupMySQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
        }
    }

    /// Get or create the global MySQL connection pool
    async fn get_pool() -> &'static mysql_async::Pool {
        MYSQL_POOL
            .get_or_init(|| async {
                let url = std::env::var("TOASTY_TEST_MYSQL_URL")
                    .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());
                mysql_async::Pool::new(url.as_str())
            })
            .await
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
            .map_err(|e| toasty::Error::msg(format!("MySQL cleanup failed: {e}")))
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
        use mysql_async::prelude::*;

        let full_table_name = format!("{}{}", self.isolation.table_prefix(), table);

        // Build WHERE clause from filter
        let mut where_conditions = Vec::new();
        let mut mysql_params = Vec::new();

        for (col_name, value) in filter {
            where_conditions.push(format!("{col_name} = ?"));

            // Convert stmt::Value to MySQL parameter
            match value {
                toasty_core::stmt::Value::String(s) => {
                    mysql_params.push(mysql_async::Value::Bytes(s.into_bytes()))
                }
                toasty_core::stmt::Value::I64(i) => mysql_params.push(mysql_async::Value::Int(i)),
                toasty_core::stmt::Value::U64(u) => mysql_params.push(mysql_async::Value::UInt(u)),
                toasty_core::stmt::Value::I32(i) => {
                    mysql_params.push(mysql_async::Value::Int(i as i64))
                }
                toasty_core::stmt::Value::I16(i) => {
                    mysql_params.push(mysql_async::Value::Int(i as i64))
                }
                toasty_core::stmt::Value::I8(i) => {
                    mysql_params.push(mysql_async::Value::Int(i as i64))
                }
                toasty_core::stmt::Value::U32(u) => {
                    mysql_params.push(mysql_async::Value::UInt(u as u64))
                }
                toasty_core::stmt::Value::U16(u) => {
                    mysql_params.push(mysql_async::Value::UInt(u as u64))
                }
                toasty_core::stmt::Value::U8(u) => {
                    mysql_params.push(mysql_async::Value::UInt(u as u64))
                }
                toasty_core::stmt::Value::Bool(b) => {
                    mysql_params.push(mysql_async::Value::Int(if b { 1 } else { 0 }))
                }
                toasty_core::stmt::Value::Id(id) => {
                    mysql_params.push(mysql_async::Value::Bytes(id.to_string().into_bytes()))
                }
                _ => todo!("Unsupported filter value type for MySQL: {value:?}"),
            }
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        let query = format!("SELECT {column} FROM {full_table_name}{where_clause}");

        // Get connection from cached pool
        let pool = Self::get_pool().await;
        let mut conn = pool
            .get_conn()
            .await
            .unwrap_or_else(|e| panic!("MySQL connection failed: {e}"));

        let mut result = conn
            .exec_iter(&query, mysql_params)
            .await
            .unwrap_or_else(|e| panic!("MySQL query failed: {e}"));

        if let Ok(Some(row)) = result.next().await {
            let stmt_value = self.mysql_row_to_stmt_value(&row, 0)?;
            stmt_value
                .try_into()
                .map_err(|e: toasty_core::Error| panic!("Validation failed: {e}"))
        } else {
            panic!("No rows found")
        }
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
         WHERE table_schema = DATABASE() AND table_name LIKE '{my_prefix}%'"
        ))
        .await?;

    // Drop each table
    for table_name in rows {
        let query = format!("DROP TABLE IF EXISTS {table_name}");
        let _ = conn.query_drop(&query).await; // Ignore individual table drop errors
    }

    drop(conn);
    pool.disconnect().await?;
    Ok(())
}

impl SetupMySQL {
    fn mysql_row_to_stmt_value(
        &self,
        row: &mysql_async::Row,
        col: usize,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use mysql_async::Value;

        let value = row
            .as_ref(col)
            .ok_or_else(|| toasty::Error::msg(format!("MySQL column {col} not found")))?;

        match value {
            Value::NULL => Ok(toasty_core::stmt::Value::Null),
            Value::Bytes(bytes) => {
                let text = String::from_utf8(bytes.clone()).map_err(|e| {
                    toasty::Error::msg(format!("MySQL bytes to string conversion failed: {e}"))
                })?;
                Ok(toasty_core::stmt::Value::String(text))
            }
            Value::Int(i) => Ok(toasty_core::stmt::Value::I64(*i)),
            Value::UInt(u) => Ok(toasty_core::stmt::Value::U64(*u)),
            _ => todo!(
                "MySQL value type conversion not yet implemented: {:?}",
                value
            ),
        }
    }
}
