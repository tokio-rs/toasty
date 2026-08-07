#![warn(missing_docs)]
#![allow(clippy::needless_range_loop)]

//! Toasty driver for [MySQL](https://www.mysql.com/) using
//! [SQLx](https://docs.rs/sqlx).
//!
//! # Examples
//!
//! ```no_run
//! use toasty_driver_mysql::MySQL;
//!
//! let driver = MySQL::new("mysql://localhost/mydb").unwrap();
//! ```

mod value;
pub(crate) use value::Value;

use async_trait::async_trait;
use sqlx_core::{
    connection::{ConnectOptions as _, Connection as SqlxConnection},
    row::Row,
    sql_str::AssertSqlSafe,
};
use sqlx_mysql::{MySqlArguments, MySqlConnectOptions, MySqlConnection, MySqlDatabaseError};
use std::{borrow::Cow, cell::Cell, sync::Arc};
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, ConnectContext, ConnectionUrl, Driver, ExecResponse, Operation, QueryLogConfig,
        log::QueryLog,
        operation::{RawSqlRet, Transaction, TransactionMode},
    },
    schema::db::{self, Table},
    stmt::{self, ValueRecord},
};
use toasty_sql::{self as sql};

enum SqlReturn {
    Count {
        last_insert_id_hack: Option<u64>,
        sql_is_insert: bool,
    },
    Infer,
    Types(Vec<stmt::Type>),
}

/// Classifies a SQLx MySQL error into a Toasty error.
///
/// Transport and protocol failures become `ConnectionLost`. MySQL
/// errors with numbers that Toasty understands become typed errors.
/// Everything else becomes `DriverOperationFailed`.
fn classify_mysql_error(e: sqlx_core::Error) -> toasty_core::Error {
    match e {
        error @ (sqlx_core::Error::Io(_)
        | sqlx_core::Error::Protocol(_)
        | sqlx_core::Error::WorkerCrashed) => toasty_core::Error::connection_lost(error),
        sqlx_core::Error::Database(database_error) => {
            let mysql_error = database_error
                .try_downcast_ref::<MySqlDatabaseError>()
                .expect("SQLx returned a non-MySQL database error from a MySQL connection");
            let number = mysql_error.number();
            let message = mysql_error.message().to_owned();

            match number {
                1213 => toasty_core::Error::serialization_failure(message),
                1792 => toasty_core::Error::read_only_transaction(message),
                _ => toasty_core::Error::driver_operation_failed(sqlx_core::Error::Database(
                    database_error,
                )),
            }
        }
        other => toasty_core::Error::driver_operation_failed(other),
    }
}

/// Classifies a SQLx error and records whether the connection is still usable.
fn record_mysql_err(valid: &Cell<bool>, e: sqlx_core::Error) -> toasty_core::Error {
    let err = classify_mysql_error(e);
    if err.is_connection_lost() {
        valid.set(false);
    }
    err
}

/// A MySQL [`Driver`] that connects through SQLx.
///
/// # Examples
///
/// ```no_run
/// use toasty_driver_mysql::MySQL;
///
/// let driver = MySQL::new("mysql://localhost/mydb").unwrap();
/// ```
#[derive(Debug)]
pub struct MySQL {
    url: String,
    opts: MySqlConnectOptions,
}

impl MySQL {
    /// Creates a MySQL driver from a SQLx connection URL.
    ///
    /// The URL must use the `mysql` scheme and include a database path, such as
    /// `mysql://user:pass@host:3306/dbname`.
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url_str = url.into();
        let url = ConnectionUrl::parse(&url_str)?;

        if !url.has_scheme("mysql") {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "connection url does not have a `mysql` scheme; url={}",
                url.as_str()
            )));
        }

        url.host()?.ok_or_else(|| {
            toasty_core::Error::invalid_connection_url(format!(
                "missing host in connection URL; url={}",
                url.as_str()
            ))
        })?;

        if url.path().is_empty() {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "no database specified - missing path in connection URL; url={}",
                url.as_str()
            )));
        }

        let opts = url
            .as_str()
            .parse::<MySqlConnectOptions>()
            .map_err(toasty_core::Error::driver_operation_failed)?
            .disable_statement_logging();

        Ok(Self { url: url_str, opts })
    }
}

#[async_trait]
impl Driver for MySQL {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.url)
    }

    fn capability(&self) -> &'static Capability {
        &Capability::MYSQL
    }

    async fn connect(
        &self,
        cx: &ConnectContext,
    ) -> Result<Box<dyn toasty_core::driver::Connection>> {
        let conn = MySqlConnection::connect_with(&self.opts)
            .await
            .map_err(classify_mysql_error)?;
        let mut connection = Connection::new(conn);
        connection.query_log = cx.query_log;
        Ok(Box::new(connection))
    }

    async fn reset_db(&self) -> Result<()> {
        let mut conn = MySqlConnection::connect_with(&self.opts)
            .await
            .map_err(classify_mysql_error)?;
        let dbname = self.opts.get_database().ok_or_else(|| {
            toasty_core::Error::invalid_connection_url("no database name configured")
        })?;

        let dbname = format!("`{}`", dbname.replace('`', "``"));

        sqlx_core::raw_sql::raw_sql(AssertSqlSafe(format!("DROP DATABASE IF EXISTS {dbname}")))
            .execute(&mut conn)
            .await
            .map_err(classify_mysql_error)?;
        sqlx_core::raw_sql::raw_sql(AssertSqlSafe(format!("CREATE DATABASE {dbname}")))
            .execute(&mut conn)
            .await
            .map_err(classify_mysql_error)?;
        sqlx_core::raw_sql::raw_sql(AssertSqlSafe(format!("USE {dbname}")))
            .execute(&mut conn)
            .await
            .map_err(classify_mysql_error)?;

        Ok(())
    }
}

/// An open connection to a MySQL database.
#[derive(Debug)]
pub struct Connection {
    conn: MySqlConnection,
    /// Set to `false` after a connection-level failure. SQLx does not expose a
    /// passive validity flag, so the driver records one for [`is_valid`].
    valid: Cell<bool>,
    query_log: QueryLogConfig,
}

impl Connection {
    /// Wraps an existing SQLx [`MySqlConnection`] as a Toasty connection.
    pub fn new(conn: MySqlConnection) -> Self {
        Self {
            conn,
            valid: Cell::new(true),
            query_log: QueryLogConfig::default(),
        }
    }

    async fn exec_sql(
        &mut self,
        sql_as_str: &str,
        args: MySqlArguments,
        ret: SqlReturn,
        log: &mut QueryLog<'_>,
    ) -> Result<ExecResponse> {
        if let SqlReturn::Count {
            last_insert_id_hack,
            sql_is_insert,
        } = ret
        {
            let result = sqlx_core::query::query_with(AssertSqlSafe(sql_as_str), args)
                .execute(&mut self.conn)
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))?;
            let count = result.rows_affected();

            if let Some(num_rows) = last_insert_id_hack {
                assert!(
                    sql_is_insert,
                    "last_insert_id_hack should only be used with INSERT statements"
                );

                let first_id = result.last_insert_id();
                let results = (0..num_rows).map(move |offset| {
                    let id = first_id + offset;
                    Ok(ValueRecord::from_vec(vec![stmt::Value::U64(id)]))
                });

                log.rows(num_rows);
                return Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
                    results,
                )));
            }

            return Ok(ExecResponse::count(count));
        }

        let rows = sqlx_core::query::query_with(AssertSqlSafe(sql_as_str), args)
            .fetch_all(&mut self.conn)
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        log.rows(rows.len() as u64);
        let mut records = Vec::with_capacity(rows.len());

        for row in rows {
            let mut values = Vec::with_capacity(row.len());

            match &ret {
                SqlReturn::Count { .. } => unreachable!(),
                SqlReturn::Infer => {
                    for i in 0..row.len() {
                        let column = row.column(i);
                        values.push(
                            Value::from_sql_infer(i, &row, column)
                                .map_err(|e| record_mysql_err(&self.valid, e))?
                                .into_inner(),
                        );
                    }
                }
                SqlReturn::Types(returning) => {
                    assert_eq!(
                        row.len(),
                        returning.len(),
                        "row={row:#?}; returning={returning:#?}"
                    );

                    for i in 0..row.len() {
                        let column = row.column(i);
                        values.push(
                            Value::from_sql(i, &row, column, &returning[i])
                                .map_err(|e| record_mysql_err(&self.valid, e))?
                                .into_inner(),
                        );
                    }
                }
            }

            records.push(Ok(ValueRecord::from_vec(values)));
        }

        Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
            records.into_iter(),
        )))
    }

    /// Creates a table and its indices from a schema definition.
    pub async fn create_table(&mut self, schema: &db::Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);
        let statement =
            serializer.serialize(&sql::Statement::create_table(table, &Capability::MYSQL));

        sqlx_core::query::query(AssertSqlSafe(statement))
            .execute(&mut self.conn)
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let statement = serializer.serialize(&sql::Statement::create_index(index));
            sqlx_core::query::query(AssertSqlSafe(statement))
                .execute(&mut self.conn)
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))?;
        }

        Ok(())
    }
}

impl From<MySqlConnection> for Connection {
    fn from(conn: MySqlConnection) -> Self {
        Self::new(conn)
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        tracing::trace!(driver = "mysql", op = %op.name(), "driver exec");

        let (sql, typed_params, ret, last_insert_id_hack) = match op {
            Operation::QuerySql(op) => (
                sql::Statement::from(op.stmt),
                op.params,
                op.ret,
                op.last_insert_id_hack,
            ),
            Operation::RawSql(op) => {
                let ret = match op.ret {
                    RawSqlRet::None => SqlReturn::Count {
                        last_insert_id_hack: None,
                        sql_is_insert: false,
                    },
                    RawSqlRet::Infer => SqlReturn::Infer,
                    RawSqlRet::Types(types) => SqlReturn::Types(types),
                };
                let mut log = QueryLog::sql(
                    &self.query_log,
                    "mysql",
                    &op.sql,
                    op.params.iter().map(|tv| &tv.value),
                );
                let mut args = MySqlArguments::default();
                for param in op.params {
                    Value::from(param.value)
                        .add_to(&mut args)
                        .map_err(|e| record_mysql_err(&self.valid, e))?;
                }
                let result = self.exec_sql(&op.sql, args, ret, &mut log).await;
                log.finish(&result);
                return result;
            }
            Operation::Transaction(op) => {
                if let Transaction::Start {
                    mode: mode @ (TransactionMode::Immediate | TransactionMode::Exclusive),
                    ..
                } = &op
                {
                    return Err(toasty_core::Error::unsupported_feature(format!(
                        "MySQL does not support TransactionMode::{mode:?}"
                    )));
                }
                let statement = sql::Serializer::mysql(&schema.db).serialize_transaction(&op);
                sqlx_core::raw_sql::raw_sql(AssertSqlSafe(statement))
                    .execute(&mut self.conn)
                    .await
                    .map_err(|e| record_mysql_err(&self.valid, e))?;
                return Ok(ExecResponse::count(0));
            }
            op => todo!("op={op:#?}"),
        };

        let (sql_as_str, arg_order) =
            sql::Serializer::mysql(&schema.db).serialize_with_arg_order(&sql);

        let mut log = QueryLog::sql(
            &self.query_log,
            "mysql",
            &sql_as_str,
            arg_order.iter().map(|&pos| &typed_params[pos].value),
        );

        // MySQL uses positional `?` placeholders, so parameters must follow the
        // order in which their `Expr::Arg(n)` values appear in serialized SQL.
        let mut remaining = vec![0usize; typed_params.len()];
        for &pos in &arg_order {
            remaining[pos] += 1;
        }
        let mut values = typed_params
            .into_iter()
            .map(|param| Some(param.value))
            .collect::<Vec<_>>();
        let mut args = MySqlArguments::default();
        for pos in arg_order {
            remaining[pos] -= 1;
            let value = if remaining[pos] == 0 {
                values[pos].take().expect("MySQL parameter already moved")
            } else {
                values[pos]
                    .as_ref()
                    .expect("MySQL parameter missing")
                    .clone()
            };
            Value::from(value)
                .add_to(&mut args)
                .map_err(|e| record_mysql_err(&self.valid, e))?;
        }

        let ret = match ret {
            Some(types) => SqlReturn::Types(types),
            None => SqlReturn::Count {
                last_insert_id_hack,
                sql_is_insert: matches!(sql, sql::Statement::Insert(_)),
            },
        };

        let result = self.exec_sql(&sql_as_str, args, ret, &mut log).await;
        log.finish(&result);
        result
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.db.tables {
            tracing::debug!(table = %table.name, "creating table");
            self.create_table(&schema.db, table).await?;
        }
        Ok(())
    }

    async fn applied_migrations(
        &mut self,
    ) -> Result<Vec<toasty_core::schema::db::AppliedMigration>> {
        sqlx_core::query::query(
            "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id BIGINT UNSIGNED PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
        )
        .execute(&mut self.conn)
        .await
        .map_err(|e| record_mysql_err(&self.valid, e))?;

        let ids = sqlx_core::query_scalar::query_scalar::<sqlx_mysql::MySql, u64>(
            "SELECT id FROM __toasty_migrations ORDER BY applied_at",
        )
        .fetch_all(&mut self.conn)
        .await
        .map_err(|e| record_mysql_err(&self.valid, e))?;

        Ok(ids
            .into_iter()
            .map(toasty_core::schema::db::AppliedMigration::new)
            .collect())
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: &str,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        tracing::info!(id, name, "applying migration");
        sqlx_core::query::query(
            "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id BIGINT UNSIGNED PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
        )
        .execute(&mut self.conn)
        .await
        .map_err(|e| record_mysql_err(&self.valid, e))?;

        let mut transaction = self
            .conn
            .begin()
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        for statement in migration.statements() {
            if let Err(error) = sqlx_core::raw_sql::raw_sql(AssertSqlSafe(statement))
                .execute(&mut *transaction)
                .await
            {
                let error = record_mysql_err(&self.valid, error);
                transaction
                    .rollback()
                    .await
                    .map_err(|e| record_mysql_err(&self.valid, e))?;
                return Err(error);
            }
        }

        if let Err(error) = sqlx_core::query::query(
            "INSERT INTO __toasty_migrations (id, name, applied_at) VALUES (?, ?, NOW())",
        )
        .bind(id)
        .bind(name)
        .execute(&mut *transaction)
        .await
        {
            let error = record_mysql_err(&self.valid, error);
            transaction
                .rollback()
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))?;
            return Err(error);
        }

        transaction
            .commit()
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;
        Ok(())
    }

    fn is_valid(&self) -> bool {
        self.valid.get()
    }

    async fn ping(&mut self) -> Result<()> {
        match self.conn.ping().await {
            Ok(()) => Ok(()),
            Err(error) => {
                self.valid.set(false);
                Err(toasty_core::Error::connection_lost(error))
            }
        }
    }
}
