use crate::{capability, PostgreSQL};
use postgres::types::ToSql;
use toasty_core::{
    driver::{Capability, TestDriver},
    stmt, Result,
};

impl TestDriver for PostgreSQL {
    const CAPABILITY: Capability = capability::CAPABILITY;

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: std::collections::HashMap<String, stmt::Value>,
    ) -> Result<stmt::Value> {
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

        let query = format!("SELECT {column} FROM {table}{where_clause}");

        // For simplicity, use string parameters for now
        let string_params: Vec<&str> = pg_params.iter().map(|s| s.as_str()).collect();
        let params_refs: Vec<&(dyn ToSql + Sync)> = string_params
            .iter()
            .map(|s| s as &(dyn ToSql + Sync))
            .collect();

        let row = self
            .client
            .query_one(&query, &params_refs)
            .await
            .map_err(|e| anyhow::anyhow!("Query failed: {e}"))?;

        // Convert PostgreSQL result directly to stmt::Value
        pg_row_to_stmt_value(&row, 0)
    }
}

fn pg_row_to_stmt_value(row: &tokio_postgres::Row, col: usize) -> Result<stmt::Value> {
    use postgres::types::Type;

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
