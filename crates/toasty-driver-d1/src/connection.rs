use std::{fmt, sync::Arc};

use async_trait::async_trait;
use toasty_core::{
    Schema,
    driver::{
        ExecResponse, QueryLogConfig,
        log::QueryLog,
        operation::{AtomicSqlBatch, Operation, QuerySql, RawSql, RawSqlRet, TypedValue},
    },
    schema::db::{AppliedMigration, Migration},
    stmt,
};
use worker::{
    D1Database, D1PreparedStatement,
    send::{IntoSendFuture, SendWrapper},
};

use crate::{
    error,
    value::{self, wasm as codec},
};

enum SqlReturn {
    Count,
    Infer,
    Types(Vec<stmt::Type>),
}

pub(crate) struct Connection {
    database: SendWrapper<D1Database>,
    query_log: QueryLogConfig,
}

impl Connection {
    pub(crate) fn new(database: SendWrapper<D1Database>, query_log: QueryLogConfig) -> Self {
        Self {
            database,
            query_log,
        }
    }

    async fn exec_query_sql(
        &self,
        schema: &Schema,
        operation: QuerySql,
    ) -> toasty_core::Result<ExecResponse> {
        if operation.last_insert_id_hack.is_some() {
            return Err(error::unsupported("MySQL last-insert-id workaround"));
        }

        validate_patterns(&operation.stmt, &operation.params)?;

        let statement = toasty_sql::Statement::from(operation.stmt);
        let sql = toasty_sql::Serializer::sqlite(&schema.db).serialize(&statement);
        let ret = operation
            .ret
            .map(SqlReturn::Types)
            .unwrap_or(SqlReturn::Count);
        self.exec_sql(&sql, operation.params, ret, "query_sql")
            .await
    }

    async fn exec_raw_sql(&self, operation: RawSql) -> toasty_core::Result<ExecResponse> {
        let ret = match operation.ret {
            RawSqlRet::None => SqlReturn::Count,
            RawSqlRet::Infer => SqlReturn::Infer,
            RawSqlRet::Types(types) => SqlReturn::Types(types),
        };
        self.exec_sql(&operation.sql, operation.params, ret, "raw_sql")
            .await
    }

    async fn exec_sql(
        &self,
        sql: &str,
        params: Vec<TypedValue>,
        ret: SqlReturn,
        operation: &str,
    ) -> toasty_core::Result<ExecResponse> {
        value::validate_sql(sql)?;
        value::validate_parameter_count(params.len())?;

        let log = QueryLog::sql(
            &self.query_log,
            "d1",
            sql,
            params.iter().map(|param| &param.value),
        );
        let result = self.exec_sql_inner(sql, params, ret, operation).await;
        log.finish(&result);
        result
    }

    async fn exec_sql_inner(
        &self,
        sql: &str,
        params: Vec<TypedValue>,
        ret: SqlReturn,
        operation: &str,
    ) -> toasty_core::Result<ExecResponse> {
        let values = params
            .into_iter()
            .map(|param| codec::bind(param.value))
            .collect::<toasty_core::Result<Vec<_>>>()?;
        let statement = self
            .database
            .prepare(sql)
            .bind(&values)
            .map_err(|error| error::worker(operation, error))?;

        match ret {
            SqlReturn::Count => execute_count(&statement, operation).await,
            SqlReturn::Infer => execute_rows(&statement, None, operation).await,
            SqlReturn::Types(types) => execute_rows(&statement, Some(&types), operation).await,
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish_non_exhaustive()
    }
}

async fn execute_count(
    statement: &D1PreparedStatement,
    operation: &str,
) -> toasty_core::Result<ExecResponse> {
    let result = statement
        .run()
        .into_send()
        .await
        .map_err(|error| error::worker(operation, error))?;
    ensure_success(&result, operation)?;
    let changes = result
        .meta()
        .map_err(|error| error::worker(operation, error))?
        .and_then(|meta| meta.changes)
        .unwrap_or(0);
    Ok(ExecResponse::count(changes as u64))
}

async fn execute_rows(
    statement: &D1PreparedStatement,
    types: Option<&[stmt::Type]>,
    operation: &str,
) -> toasty_core::Result<ExecResponse> {
    let rows = statement
        .raw_js_value()
        .into_send()
        .await
        .map_err(|error| error::worker(operation, error))?;
    let mut decoded = Vec::with_capacity(rows.len());
    for row in rows {
        let row = codec::row(row)?;
        if let Some(types) = types
            && row.length() as usize != types.len()
        {
            return Err(toasty_core::Error::invalid_result(format!(
                "D1 row has {} columns; expected {}",
                row.length(),
                types.len()
            )));
        }

        let values = row
            .iter()
            .enumerate()
            .map(|(index, item)| match types {
                Some(types) => codec::decode_typed(item, &types[index], index),
                None => codec::decode_infer(item, index),
            })
            .collect::<toasty_core::Result<Vec<_>>>()?;
        decoded.push(stmt::ValueRecord::from_vec(values).into());
    }

    Ok(ExecResponse::value_stream(stmt::ValueStream::from_vec(
        decoded,
    )))
}

fn ensure_success(result: &worker::D1Result, operation: &str) -> toasty_core::Result<()> {
    if result.success() {
        Ok(())
    } else {
        Err(error::result(
            operation,
            result.error().unwrap_or_else(|| "unknown D1 error".into()),
        ))
    }
}

fn validate_patterns(
    statement: &stmt::Statement,
    params: &[TypedValue],
) -> toasty_core::Result<()> {
    let mut validation = Ok(());
    stmt::visit::for_each_expr(statement, |expression| {
        if validation.is_err() {
            return;
        }
        let pattern = match expression {
            stmt::Expr::Like(expression) => expression.pattern.as_ref(),
            stmt::Expr::StartsWith(expression) => expression.prefix.as_ref(),
            _ => return,
        };
        let value = match pattern {
            stmt::Expr::Arg(argument) if argument.nesting == 0 => {
                params.get(argument.position).map(|param| &param.value)
            }
            stmt::Expr::Value(value) | stmt::Expr::Static(value) => Some(value),
            _ => None,
        };
        if let Some(stmt::Value::String(pattern)) = value {
            validation = value::validate_pattern(pattern);
        }
    });
    validation
}

fn decode_batch_result(
    result: &worker::D1Result,
    types: Option<&[stmt::Type]>,
    statement_index: usize,
) -> toasty_core::Result<ExecResponse> {
    ensure_success(result, &format!("batch statement {statement_index}"))?;
    let Some(types) = types else {
        let changes = result
            .meta()
            .map_err(|error| error::worker("batch metadata", error))?
            .and_then(|meta| meta.changes)
            .unwrap_or(0);
        return Ok(ExecResponse::count(changes as u64));
    };

    let rows: Vec<serde_json::Value> = result
        .results()
        .map_err(|error| error::worker("batch results", error))?;
    let mut decoded = Vec::with_capacity(rows.len());
    for row in rows {
        let serde_json::Value::Object(mut row) = row else {
            return Err(toasty_core::Error::invalid_result(format!(
                "D1 batch statement {statement_index} returned a non-object row"
            )));
        };
        let mut values = Vec::with_capacity(types.len());
        for (column_index, ty) in types.iter().enumerate() {
            let alias = format!("column{}", column_index + 1);
            let item = row.remove(&alias).ok_or_else(|| {
                toasty_core::Error::invalid_result(format!(
                    "D1 batch statement {statement_index} row is missing alias {alias}"
                ))
            })?;
            let item = worker::d1::serde_wasm_bindgen::to_value(&item)
                .map_err(|error| error::worker("batch value conversion", error.into()))?;
            values.push(codec::decode_typed(item, ty, column_index)?);
        }
        decoded.push(stmt::ValueRecord::from_vec(values).into());
    }

    Ok(ExecResponse::value_stream(stmt::ValueStream::from_vec(
        decoded,
    )))
}

#[async_trait]
impl toasty_core::Connection for Connection {
    async fn exec(
        &mut self,
        schema: &Arc<Schema>,
        operation: Operation,
    ) -> toasty_core::Result<ExecResponse> {
        tracing::trace!(driver = "d1", op = %operation.name(), "driver exec");
        match operation {
            Operation::QuerySql(operation) => self.exec_query_sql(schema, operation).await,
            Operation::RawSql(operation) => self.exec_raw_sql(operation).await,
            Operation::Transaction(_) => Err(error::unsupported("transaction")),
            operation => Err(error::unsupported(operation.name())),
        }
    }

    async fn exec_atomic_batch(
        &mut self,
        schema: &Arc<Schema>,
        batch: AtomicSqlBatch,
    ) -> toasty_core::Result<Vec<ExecResponse>> {
        let mut statements = Vec::with_capacity(batch.operations.len());
        let mut returns = Vec::with_capacity(batch.operations.len());

        for (index, operation) in batch.operations.into_iter().enumerate() {
            if operation.last_insert_id_hack.is_some() {
                return Err(error::unsupported("MySQL last-insert-id workaround"));
            }
            value::validate_parameter_count(operation.params.len()).map_err(|error| {
                error.context(toasty_core::Error::from_args(format_args!(
                    "D1 batch statement {index} is invalid"
                )))
            })?;
            validate_patterns(&operation.stmt, &operation.params)?;

            let statement = toasty_sql::Statement::from(operation.stmt);
            let sql = toasty_sql::Serializer::sqlite(&schema.db).serialize(&statement);
            value::validate_sql(&sql)?;
            let values = operation
                .params
                .into_iter()
                .map(|param| codec::bind(param.value))
                .collect::<toasty_core::Result<Vec<_>>>()?;
            let statement =
                self.database.prepare(sql).bind(&values).map_err(|error| {
                    error::worker(&format!("batch statement {index} bind"), error)
                })?;
            statements.push(statement);
            returns.push(operation.ret);
        }

        tracing::trace!(
            driver = "d1",
            statements = statements.len(),
            "executing atomic SQL batch"
        );
        let results = self
            .database
            .batch(statements)
            .into_send()
            .await
            .map_err(|error| error::worker("atomic batch", error))?;
        if results.len() != returns.len() {
            return Err(toasty_core::Error::invalid_result(format!(
                "D1 atomic batch returned {} results for {} statements",
                results.len(),
                returns.len()
            )));
        }

        results
            .iter()
            .zip(&returns)
            .enumerate()
            .map(|(index, (result, types))| decode_batch_result(result, types.as_deref(), index))
            .collect()
    }

    async fn push_schema(&mut self, schema: &Schema) -> toasty_core::Result<()> {
        let serializer = toasty_sql::Serializer::sqlite(&schema.db);
        let mut statements = Vec::new();
        for table in &schema.db.tables {
            if table.columns.len() > 100 {
                return Err(toasty_core::Error::validation_failed(format!(
                    "D1 table {} has {} columns; maximum is 100",
                    table.name,
                    table.columns.len()
                )));
            }

            let sql = serializer.serialize(&toasty_sql::Statement::create_table(
                table,
                &toasty_core::driver::Capability::D1,
            ));
            value::validate_sql(&sql)?;
            statements.push(self.database.prepare(sql));
            for index in &table.indices {
                if index.primary_key {
                    continue;
                }
                let sql = serializer.serialize(&toasty_sql::Statement::create_index(index));
                value::validate_sql(&sql)?;
                statements.push(self.database.prepare(sql));
            }
        }

        if statements.is_empty() {
            return Ok(());
        }
        let statement_count = statements.len();
        let results = self
            .database
            .batch(statements)
            .into_send()
            .await
            .map_err(|error| error::worker("push schema", error))?;
        if results.len() != statement_count {
            return Err(toasty_core::Error::invalid_result(format!(
                "D1 schema batch returned {} results for {statement_count} statements",
                results.len()
            )));
        }
        for (index, result) in results.iter().enumerate() {
            ensure_success(result, &format!("push schema statement {index}"))?;
        }
        Ok(())
    }

    async fn applied_migrations(&mut self) -> toasty_core::Result<Vec<AppliedMigration>> {
        Err(error::unsupported("read applied migrations"))
    }

    async fn apply_migration(
        &mut self,
        _id: u64,
        _name: &str,
        _migration: &Migration,
    ) -> toasty_core::Result<()> {
        Err(error::unsupported("apply migration"))
    }
}
