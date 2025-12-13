use crate::{capability, MySQL};
use mysql_async::prelude::Queryable;
use toasty_core::{
    driver::{Capability, TestDriver},
    stmt, Result,
};

impl TestDriver for MySQL {
    const CAPABILITY: Capability = capability::CAPABILITY;

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: std::collections::HashMap<String, stmt::Value>,
    ) -> Result<stmt::Value> {
        // Build WHERE clause from filter
        let mut where_conditions = Vec::new();
        let mut mysql_params = Vec::new();

        for (col_name, value) in filter {
            where_conditions.push(format!("{col_name} = ?"));

            // Convert stmt::Value to MySQL parameter
            match value {
                stmt::Value::String(s) => {
                    mysql_params.push(mysql_async::Value::Bytes(s.into_bytes()))
                }
                stmt::Value::I64(i) => mysql_params.push(mysql_async::Value::Int(i)),
                stmt::Value::U64(u) => mysql_params.push(mysql_async::Value::UInt(u)),
                stmt::Value::I32(i) => mysql_params.push(mysql_async::Value::Int(i as i64)),
                stmt::Value::I16(i) => mysql_params.push(mysql_async::Value::Int(i as i64)),
                stmt::Value::I8(i) => mysql_params.push(mysql_async::Value::Int(i as i64)),
                stmt::Value::U32(u) => mysql_params.push(mysql_async::Value::UInt(u as u64)),
                stmt::Value::U16(u) => mysql_params.push(mysql_async::Value::UInt(u as u64)),
                stmt::Value::U8(u) => mysql_params.push(mysql_async::Value::UInt(u as u64)),
                stmt::Value::Bool(b) => {
                    mysql_params.push(mysql_async::Value::Int(if b { 1 } else { 0 }))
                }
                stmt::Value::Id(id) => {
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

        let query = format!("SELECT {column} FROM {table}{where_clause}");

        // Get connection from pool
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| anyhow::anyhow!("MySQL connection failed: {e}"))?;

        let mut result = conn
            .exec_iter(&query, mysql_params)
            .await
            .map_err(|e| anyhow::anyhow!("MySQL query failed: {e}"))?;

        if let Ok(Some(row)) = result.next().await {
            mysql_row_to_stmt_value(&row, 0)
        } else {
            Err(anyhow::anyhow!("No rows found"))
        }
    }
}

fn mysql_row_to_stmt_value(row: &mysql_async::Row, col: usize) -> Result<stmt::Value> {
    use mysql_async::Value;

    let value = row
        .as_ref(col)
        .ok_or_else(|| anyhow::anyhow!("MySQL column {col} not found"))?;

    match value {
        Value::NULL => Ok(stmt::Value::Null),
        Value::Bytes(bytes) => {
            let text = String::from_utf8(bytes.clone())
                .map_err(|e| anyhow::anyhow!("MySQL bytes to string conversion failed: {e}"))?;
            Ok(stmt::Value::String(text))
        }
        Value::Int(i) => Ok(stmt::Value::I64(*i)),
        Value::UInt(u) => Ok(stmt::Value::U64(*u)),
        _ => todo!(
            "MySQL value type conversion not yet implemented: {:?}",
            value
        ),
    }
}
