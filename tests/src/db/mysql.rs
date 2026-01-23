use std::collections::HashMap;
use toasty::{
    db::Connect,
    driver::{Capability, Driver},
};

use crate::{isolation::TestIsolation, Setup};

pub struct SetupMySQL {
    isolation: TestIsolation,
    // Per-test-instance pool to avoid runtime issues with static sharing
    pool: tokio::sync::OnceCell<mysql_async::Pool>,
}

impl SetupMySQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
            pool: tokio::sync::OnceCell::new(),
        }
    }

    /// Get or create the per-test-instance MySQL connection pool
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

impl Default for SetupMySQL {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupMySQL {
    fn driver(&self) -> Box<dyn Driver> {
        let url = std::env::var("TOASTY_TEST_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());
        Box::new(Connect::new(&url).unwrap())
    }

    fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let prefix = self.isolation.table_prefix();
        builder.table_name_prefix(&prefix);
    }

    fn capability(&self) -> &Capability {
        &Capability::MYSQL
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        self.cleanup_mysql_tables_impl()
            .await
            .map_err(|e| toasty::err!("MySQL cleanup failed: {e}"))
    }

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use mysql_async::prelude::Queryable;

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

        // Get connection from per-test-instance pool
        let pool = self.get_pool().await;
        let mut conn = pool
            .get_conn()
            .await
            .unwrap_or_else(|e| panic!("MySQL connection failed: {e}"));

        let mut result = conn
            .exec_iter(&query, mysql_params)
            .await
            .unwrap_or_else(|e| panic!("MySQL query failed: {e}"));

        if let Ok(Some(row)) = result.next().await {
            self.mysql_row_to_stmt_value(&row, 0)
        } else {
            panic!("No rows found")
        }
    }
}

impl SetupMySQL {
    /// Cleanup MySQL tables using the cached connection
    async fn cleanup_mysql_tables_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        use mysql_async::prelude::Queryable;

        // Reuse the cached connection pool
        let pool = self.get_pool().await;
        let mut conn = pool.get_conn().await?;

        let my_prefix = self.isolation.table_prefix();
        let escaped_prefix = my_prefix
            .replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%");

        // Query for tables that belong to this test
        let rows: Vec<String> = conn
            .query(format!(
                "SELECT table_name FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name LIKE '{escaped_prefix}%'"
            ))
            .await?;

        // Drop each table
        for table_name in rows {
            let query = format!("DROP TABLE IF EXISTS {table_name}");
            let _ = conn.query_drop(&query).await; // Ignore individual table drop errors
        }

        Ok(())
    }

    fn mysql_row_to_stmt_value(
        &self,
        row: &mysql_async::Row,
        col: usize,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use mysql_async::Value;

        let value = row
            .as_ref(col)
            .ok_or_else(|| toasty::err!("MySQL column {col} not found"))?;

        match value {
            Value::NULL => Ok(toasty_core::stmt::Value::Null),
            Value::Bytes(bytes) => {
                let text = String::from_utf8(bytes.clone())
                    .map_err(|e| toasty::err!("MySQL bytes to string conversion failed: {e}"))?;
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
