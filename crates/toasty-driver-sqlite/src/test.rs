use crate::{capability, Sqlite};
use rusqlite::ToSql;
use toasty_core::{
    driver::{Capability, TestDriver},
    stmt, Result,
};

impl TestDriver for Sqlite {
    const CAPABILITY: Capability = capability::CAPABILITY;

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: std::collections::HashMap<String, stmt::Value>,
    ) -> Result<stmt::Value> {
        // Build WHERE clause from filter
        let mut where_conditions = Vec::new();
        let mut sqlite_params = Vec::new();

        for (col_name, value) in filter {
            where_conditions.push(format!("{col_name} = ?"));

            // Convert stmt::Value to SQLite parameter
            match value {
                stmt::Value::String(s) => sqlite_params.push(s),
                stmt::Value::I64(i) => sqlite_params.push(i.to_string()),
                stmt::Value::U64(u) => sqlite_params.push(u.to_string()),
                stmt::Value::I32(i) => sqlite_params.push(i.to_string()),
                stmt::Value::I16(i) => sqlite_params.push(i.to_string()),
                stmt::Value::I8(i) => sqlite_params.push(i.to_string()),
                stmt::Value::U32(u) => sqlite_params.push(u.to_string()),
                stmt::Value::U16(u) => sqlite_params.push(u.to_string()),
                stmt::Value::U8(u) => sqlite_params.push(u.to_string()),
                stmt::Value::Bool(b) => {
                    sqlite_params.push(if b { "1".to_string() } else { "0".to_string() })
                }
                stmt::Value::Id(id) => sqlite_params.push(id.to_string()),
                _ => todo!("Unsupported filter value type for SQLite: {value:?}"),
            }
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        let query = format!("SELECT {column} FROM {table}{where_clause}");

        // Use the connection for access
        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {e}"))?;

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| anyhow::anyhow!("Failed to prepare query: {e}"))?;

        let string_params: Vec<&str> = sqlite_params.iter().map(|s| s.as_str()).collect();
        let params_refs: Vec<&dyn ToSql> = string_params
            .iter()
            .map(|s| s as &dyn ToSql)
            .collect();

        let mut rows = stmt
            .query(&params_refs[..])
            .map_err(|e| anyhow::anyhow!("SQLite query failed: {e}"))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| anyhow::anyhow!("SQLite row fetch failed: {e}"))?
        {
            sqlite_row_to_stmt_value(row, 0)
        } else {
            Err(anyhow::anyhow!("No rows found"))
        }
    }
}

fn sqlite_row_to_stmt_value(
    row: &rusqlite::Row,
    col: usize,
) -> Result<stmt::Value> {
    use rusqlite::types::ValueRef;

    let value_ref = row
        .get_ref(col)
        .map_err(|e| anyhow::anyhow!("SQLite column access failed: {e}"))?;

    match value_ref {
        ValueRef::Integer(i) => Ok(stmt::Value::I64(i)),
        ValueRef::Text(s) => {
            let text = std::str::from_utf8(s)
                .map_err(|e| anyhow::anyhow!("SQLite text conversion failed: {e}"))?;
            Ok(stmt::Value::String(text.to_string()))
        }
        ValueRef::Null => Ok(stmt::Value::Null),
        _ => todo!(
            "SQLite value type conversion not yet implemented: {:?}",
            value_ref
        ),
    }
}
