use std::{fmt, sync::Arc};

use async_trait::async_trait;
use toasty_core::{
    Schema,
    driver::{
        ExecResponse, QueryLogConfig,
        log::QueryLog,
        operation::{Operation, QuerySql, RawSql, RawSqlRet, TypedValue},
    },
    schema::db::{AppliedMigration, Migration},
    stmt,
};
use worker::{
    D1Database, D1PreparedStatement,
    send::{IntoSendFuture, SendWrapper},
};

use crate::{error, value::wasm as value};

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
        if params.len() > 100 {
            return Err(toasty_core::Error::validation_failed(format!(
                "D1 statement has {} bind parameters; maximum is 100",
                params.len()
            )));
        }

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
            .map(|param| value::bind(param.value))
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
        let row = value::row(row)?;
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
                Some(types) => value::decode_typed(item, &types[index], index),
                None => value::decode_infer(item, index),
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

    async fn push_schema(&mut self, _schema: &Schema) -> toasty_core::Result<()> {
        Err(error::unsupported("push schema"))
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
