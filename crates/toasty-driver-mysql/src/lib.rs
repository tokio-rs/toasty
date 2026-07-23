#![warn(missing_docs)]
#![allow(clippy::needless_range_loop)]

//! Toasty driver for [MySQL](https://www.mysql.com/) using
//! [`mysql_async`](https://docs.rs/mysql_async).
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
use mysql_async::{Conn, OptsBuilder, prelude::Queryable};
use std::{borrow::Cow, cell::Cell, sync::Arc};
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, ConnectContext, Driver, ExecResponse, Operation, QueryLogConfig,
        log::QueryLog,
        operation::{RawSqlRet, Transaction, TransactionMode},
    },
    schema::{
        db::{self, Migration, Table},
        diff,
    },
    stmt::{self, ValueRecord},
};
use toasty_sql::{self as sql};
use url::Url;

enum SqlReturn {
    Count {
        last_insert_id_hack: Option<u64>,
        sql_is_insert: bool,
    },
    Infer,
    Types(Vec<stmt::Type>),
}

/// Classifies a `mysql_async::Error` into a Toasty error.
///
/// `Error::Io` (any TCP/TLS-level fault) and the IO-shaped `Driver`
/// variants (`ConnectionClosed`, `PoolDisconnected`) become
/// `ConnectionLost`. `Server` errors with known SQLSTATE codes are
/// mapped to typed variants. Everything else is
/// `DriverOperationFailed`.
fn classify_mysql_error(e: mysql_async::Error) -> toasty_core::Error {
    use mysql_async::{DriverError, Error};
    match e {
        Error::Io(_) => toasty_core::Error::connection_lost(e),
        Error::Driver(DriverError::ConnectionClosed | DriverError::PoolDisconnected) => {
            toasty_core::Error::connection_lost(e)
        }
        Error::Server(se) => match se.code {
            1213 => toasty_core::Error::serialization_failure(se.message),
            1792 => toasty_core::Error::read_only_transaction(se.message),
            _ => toasty_core::Error::driver_operation_failed(Error::Server(se)),
        },
        other => toasty_core::Error::driver_operation_failed(other),
    }
}

/// Classify a `mysql_async::Error`, also flipping the connection's
/// validity flag if the error indicates the connection is gone.
fn record_mysql_err(valid: &Cell<bool>, e: mysql_async::Error) -> toasty_core::Error {
    let err = classify_mysql_error(e);
    if err.is_connection_lost() {
        valid.set(false);
    }
    err
}

/// A MySQL [`Driver`] that connects via `mysql_async`.
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
    opts: OptsBuilder,
}

impl MySQL {
    /// Create a new MySQL driver from a connection URL.
    ///
    /// The URL must use the `mysql` scheme and include a database path, e.g.
    /// `mysql://user:pass@host:3306/dbname`.
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url_str = url.into();
        let url = Url::parse(&url_str).map_err(toasty_core::Error::driver_operation_failed)?;

        if url.scheme() != "mysql" {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "connection url does not have a `mysql` scheme; url={}",
                url
            )));
        }

        url.host_str().ok_or_else(|| {
            toasty_core::Error::invalid_connection_url(format!(
                "missing host in connection URL; url={}",
                url
            ))
        })?;

        if url.path().is_empty() {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "no database specified - missing path in connection URL; url={}",
                url
            )));
        }

        let opts = mysql_async::Opts::from_url(url.as_ref())
            .map_err(toasty_core::Error::driver_operation_failed)?;
        let opts = mysql_async::OptsBuilder::from_opts(opts).client_found_rows(true);

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
        let conn = Conn::new(self.opts.clone())
            .await
            .map_err(classify_mysql_error)?;
        let mut connection = Connection::new(conn);
        connection.query_log = cx.query_log;
        Ok(Box::new(connection))
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::MYSQL);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| sql::Serializer::mysql(stmt.schema()).serialize(stmt.statement()))
            .collect();

        Migration::new_sql_with_breakpoints(&sql_strings)
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        let mut conn = Conn::new(self.opts.clone())
            .await
            .map_err(classify_mysql_error)?;

        let dbname = conn
            .opts()
            .db_name()
            .ok_or_else(|| {
                toasty_core::Error::invalid_connection_url("no database name configured")
            })?
            .to_string();

        conn.query_drop(format!("DROP DATABASE IF EXISTS `{}`", dbname))
            .await
            .map_err(classify_mysql_error)?;

        conn.query_drop(format!("CREATE DATABASE `{}`", dbname))
            .await
            .map_err(classify_mysql_error)?;

        conn.query_drop(format!("USE `{}`", dbname))
            .await
            .map_err(classify_mysql_error)?;

        Ok(())
    }
}

/// An open connection to a MySQL database.
#[derive(Debug)]
pub struct Connection {
    conn: Conn,
    /// Set to `false` once `exec` has observed a connection-lost
    /// error. `mysql_async::Conn` does not expose a passive flag, so
    /// the driver tracks one itself. Read by [`is_valid`].
    valid: Cell<bool>,
    query_log: QueryLogConfig,
}

impl Connection {
    /// Wrap an existing [`mysql_async::Conn`] as a Toasty connection.
    pub fn new(conn: Conn) -> Self {
        Self {
            conn,
            valid: Cell::new(true),
            query_log: QueryLogConfig::default(),
        }
    }

    async fn exec_sql(
        &mut self,
        sql_as_str: &str,
        args: Vec<mysql_async::Value>,
        ret: SqlReturn,
        log: &mut QueryLog<'_>,
    ) -> Result<ExecResponse> {
        let statement = self
            .conn
            .prep(sql_as_str)
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        if let SqlReturn::Count {
            last_insert_id_hack,
            sql_is_insert,
        } = ret
        {
            let count = self
                .conn
                .exec_iter(&statement, mysql_async::Params::Positional(args))
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))?
                .affected_rows();

            if let Some(num_rows) = last_insert_id_hack {
                assert!(
                    sql_is_insert,
                    "last_insert_id_hack should only be used with INSERT statements"
                );

                let first_id: u64 = self
                    .conn
                    .query_first("SELECT LAST_INSERT_ID()")
                    .await
                    .map_err(|e| record_mysql_err(&self.valid, e))?
                    .ok_or_else(|| {
                        toasty_core::Error::driver_operation_failed(std::io::Error::other(
                            "LAST_INSERT_ID() returned no rows",
                        ))
                    })?;

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

        let rows: Vec<mysql_async::Row> = self
            .conn
            .exec(&statement, &args)
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        log.rows(rows.len() as u64);

        let results = rows.into_iter().map(move |mut row| {
            let mut results = Vec::new();

            match &ret {
                SqlReturn::Count { .. } => unreachable!(),
                SqlReturn::Infer => {
                    for i in 0..row.len() {
                        let column = row.columns()[i].clone();
                        results.push(Value::from_sql_infer(i, &mut row, &column).into_inner());
                    }
                }
                SqlReturn::Types(returning) => {
                    assert_eq!(
                        row.len(),
                        returning.len(),
                        "row={row:#?}; returning={returning:#?}"
                    );

                    for i in 0..row.len() {
                        let column = row.columns()[i].clone();
                        results.push(
                            Value::from_sql(i, &mut row, &column, &returning[i]).into_inner(),
                        );
                    }
                }
            }

            Ok(ValueRecord::from_vec(results))
        });

        Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
            results,
        )))
    }

    /// Create a table and its indices from a schema definition.
    pub async fn create_table(&mut self, schema: &db::Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);

        let sql = serializer.serialize(&sql::Statement::create_table(table, &Capability::MYSQL));

        self.conn
            .exec_drop(&sql, ())
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index));

            self.conn
                .exec_drop(&sql, ())
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))?;
        }

        Ok(())
    }
}

impl From<Conn> for Connection {
    fn from(conn: Conn) -> Self {
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
                let args = op
                    .params
                    .into_iter()
                    .map(|tv| Value::from(tv.value).into_mysql())
                    .collect();
                let result = self.exec_sql(&op.sql, args, ret, &mut log).await;
                log.finish(&result);
                return result;
            }
            Operation::Transaction(op) => {
                // MySQL has no `BEGIN IMMEDIATE` / `BEGIN EXCLUSIVE`
                // analogue; reject non-Default modes loudly rather than
                // silently dropping them at the serializer.
                if let Transaction::Start {
                    mode: mode @ (TransactionMode::Immediate | TransactionMode::Exclusive),
                    ..
                } = &op
                {
                    return Err(toasty_core::Error::unsupported_feature(format!(
                        "MySQL does not support TransactionMode::{mode:?}"
                    )));
                }
                let sql = sql::Serializer::mysql(&schema.db).serialize_transaction(&op);
                self.conn
                    .query_drop(sql)
                    .await
                    .map_err(|e| record_mysql_err(&self.valid, e))?;
                return Ok(ExecResponse::count(0));
            }
            op => todo!("op={:#?}", op),
        };

        let (sql_as_str, arg_order) =
            sql::Serializer::mysql(&schema.db).serialize_with_arg_order(&sql);

        let mut log = QueryLog::sql(
            &self.query_log,
            "mysql",
            &sql_as_str,
            arg_order.iter().map(|&pos| &typed_params[pos].value),
        );

        // MySQL uses positional `?` without indices, so params must be reordered
        // to match the order `Expr::Arg(n)` placeholders appear in the SQL.
        // Move a parameter on its final use; repeated placeholders clone only
        // the earlier occurrences that require distinct protocol values.
        let mut remaining = vec![0usize; typed_params.len()];
        for &pos in &arg_order {
            remaining[pos] += 1;
        }
        let mut values = typed_params
            .into_iter()
            .map(|param| Some(param.value))
            .collect::<Vec<_>>();
        let args = arg_order
            .into_iter()
            .map(|pos| {
                remaining[pos] -= 1;
                let value = if remaining[pos] == 0 {
                    values[pos].take().expect("MySQL parameter already moved")
                } else {
                    values[pos]
                        .as_ref()
                        .expect("MySQL parameter missing")
                        .clone()
                };
                Value::from(value).into_mysql()
            })
            .collect::<Vec<_>>();

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
        // Ensure the migrations table exists
        self.conn
            .exec_drop(
                "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id BIGINT UNSIGNED PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
                (),
            )
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        // Query all applied migrations
        let rows: Vec<u64> = self
            .conn
            .exec("SELECT id FROM __toasty_migrations ORDER BY applied_at", ())
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        Ok(rows
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
        tracing::info!(id = id, name = %name, "applying migration");
        // Ensure the migrations table exists
        self.conn
            .exec_drop(
                "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id BIGINT UNSIGNED PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
                (),
            )
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        // Start transaction
        let mut transaction = self
            .conn
            .start_transaction(Default::default())
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))?;

        // Execute each migration statement
        for statement in migration.statements() {
            if let Err(e) = transaction
                .query_drop(statement)
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))
            {
                transaction
                    .rollback()
                    .await
                    .map_err(|e| record_mysql_err(&self.valid, e))?;
                return Err(e);
            }
        }

        // Record the migration
        if let Err(e) = transaction
            .exec_drop(
                "INSERT INTO __toasty_migrations (id, name, applied_at) VALUES (?, ?, NOW())",
                (id, name),
            )
            .await
            .map_err(|e| record_mysql_err(&self.valid, e))
        {
            transaction
                .rollback()
                .await
                .map_err(|e| record_mysql_err(&self.valid, e))?;
            return Err(e);
        }

        // Commit transaction
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
        // `COM_PING` is the cheapest server round-trip in the MySQL
        // protocol. Any failure is surfaced as `connection_lost`: the
        // only meaningful outcome of a ping is "the connection is
        // alive" or "evict it." Also flip the validity flag so a
        // subsequent `is_valid` check observes the dead connection.
        match self.conn.ping().await {
            Ok(()) => Ok(()),
            Err(e) => {
                self.valid.set(false);
                Err(toasty_core::Error::connection_lost(e))
            }
        }
    }
}
