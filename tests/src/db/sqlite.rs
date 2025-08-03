use std::collections::HashMap;
use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupSqlite;

impl SetupSqlite {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SetupSqlite {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        // SQLite uses in-memory databases, so no isolation needed
        builder.connect("sqlite::memory:").await
    }

    fn capability(&self) -> &Capability {
        &Capability::SQLITE
    }

    // SQLite uses in-memory databases, so no cleanup needed
    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        Ok(())
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
        // For SQLite in-memory databases, we can't access the same database from a different connection
        // This is a limitation of the current test setup. In a real application, you'd use a file-based
        // database or share the connection. For now, we'll return an informative error.

        // However, let's try to implement this properly for when file-based databases are used
        // or for future improvements to the test infrastructure.

        // Build WHERE clause from filter
        let mut where_conditions = Vec::new();
        let mut sqlite_params = Vec::new();

        for (col_name, value) in filter {
            where_conditions.push(format!("{col_name} = ?"));

            // Convert stmt::Value to SQLite parameter
            match value {
                toasty_core::stmt::Value::String(s) => sqlite_params.push(s),
                toasty_core::stmt::Value::I64(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::U64(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::I32(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::I16(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::I8(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::U32(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::U16(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::U8(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::Bool(b) => {
                    sqlite_params.push(if b { "1".to_string() } else { "0".to_string() })
                }
                toasty_core::stmt::Value::Id(id) => sqlite_params.push(id.to_string()),
                _ => {
                    return Err(toasty::Error::msg(format!(
                        "Unsupported filter value type for SQLite: {value:?}"
                    )))
                }
            }
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        let query = format!("SELECT {column} FROM {table}{where_clause}");

        // Try to connect to a SQLite database
        // For in-memory databases, this will fail as expected
        // For file-based databases, this could work
        match std::env::var("TOASTY_TEST_SQLITE_URL") {
            Ok(url) if !url.contains(":memory:") => {
                // File-based database - we can try to connect
                use rusqlite::Connection;

                let conn = Connection::open(&url)
                    .map_err(|e| toasty::Error::msg(format!("SQLite connection failed: {e}")))?;

                let mut stmt = conn
                    .prepare(&query)
                    .map_err(|e| toasty::Error::msg(format!("SQLite prepare failed: {e}")))?;

                let string_params: Vec<&str> = sqlite_params.iter().map(|s| s.as_str()).collect();
                let params_refs: Vec<&dyn rusqlite::ToSql> = string_params
                    .iter()
                    .map(|s| s as &dyn rusqlite::ToSql)
                    .collect();

                let mut rows = stmt
                    .query(&params_refs[..])
                    .map_err(|e| toasty::Error::msg(format!("SQLite query failed: {e}")))?;

                if let Some(row) = rows
                    .next()
                    .map_err(|e| toasty::Error::msg(format!("SQLite row fetch failed: {e}")))?
                {
                    let stmt_value = self.sqlite_row_to_stmt_value(&row, 0)?;
                    stmt_value.try_into().map_err(|e: toasty_core::Error| {
                        toasty::Error::msg(format!("Validation failed: {e}"))
                    })
                } else {
                    Err(toasty::Error::msg("No rows found"))
                }
            }
            _ => {
                // In-memory database or no URL specified
                Err(toasty::Error::msg(
                    "SQLite in-memory database raw value access not yet implemented",
                ))
            }
        }
    }
}

impl SetupSqlite {
    fn sqlite_row_to_stmt_value(
        &self,
        row: &rusqlite::Row,
        col: usize,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use rusqlite::types::ValueRef;

        let value_ref = row
            .get_ref(col)
            .map_err(|e| toasty::Error::msg(format!("SQLite column access failed: {e}")))?;

        match value_ref {
            ValueRef::Integer(i) => Ok(toasty_core::stmt::Value::I64(i)),
            ValueRef::Real(f) => {
                // SQLite stores all numbers as either INTEGER or REAL
                // For our purposes, we'll convert REAL back to string to preserve precision
                Ok(toasty_core::stmt::Value::String(f.to_string()))
            }
            ValueRef::Text(s) => {
                let text = std::str::from_utf8(s).map_err(|e| {
                    toasty::Error::msg(format!("SQLite text conversion failed: {e}"))
                })?;
                Ok(toasty_core::stmt::Value::String(text.to_string()))
            }
            ValueRef::Blob(_) => Err(toasty::Error::msg("SQLite BLOB type not supported")),
            ValueRef::Null => Ok(toasty_core::stmt::Value::Null),
        }
    }
}
