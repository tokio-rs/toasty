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
use mysql_async::{
    Conn, OptsBuilder,
    prelude::{Queryable, ToValue},
};
use std::{borrow::Cow, sync::Arc};
use toasty_core::{
    Result, Schema,
    driver::{Capability, Driver, ExecResponse, Operation},
    schema::db::{self, Migration, SchemaDiff, Table},
    stmt::{self, ValueRecord},
};
use toasty_sql::{self as sql};
use url::Url;

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

    async fn connect(&self) -> Result<Box<dyn toasty_core::driver::Connection>> {
        let conn = Conn::new(self.opts.clone())
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        Ok(Box::new(Connection::new(conn)))
    }

    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration {
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
            .map_err(toasty_core::Error::driver_operation_failed)?;

        let dbname = conn
            .opts()
            .db_name()
            .ok_or_else(|| {
                toasty_core::Error::invalid_connection_url("no database name configured")
            })?
            .to_string();

        conn.query_drop(format!("DROP DATABASE IF EXISTS `{}`", dbname))
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        conn.query_drop(format!("CREATE DATABASE `{}`", dbname))
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        conn.query_drop(format!("USE `{}`", dbname))
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        Ok(())
    }
}

/// An open connection to a MySQL database.
#[derive(Debug)]
pub struct Connection {
    conn: Conn,
}

impl Connection {
    /// Wrap an existing [`mysql_async::Conn`] as a Toasty connection.
    pub fn new(conn: Conn) -> Self {
        Self { conn }
    }

    /// Create a table and its indices from a schema definition.
    pub async fn create_table(&mut self, schema: &db::Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);

        let sql = serializer.serialize(&sql::Statement::create_table(table, &Capability::MYSQL));

        self.conn
            .exec_drop(&sql, ())
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index));

            self.conn
                .exec_drop(&sql, ())
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
        }

        Ok(())
    }
}

impl From<Conn> for Connection {
    fn from(conn: Conn) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    fn is_valid(&self) -> bool {
        true
    }

    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        tracing::trace!(driver = "mysql", op = %op.name(), "driver exec");

        let (sql, typed_params, ret, last_insert_id_hack) = match op {
            Operation::QuerySql(op) => (
                sql::Statement::from(op.stmt),
                op.params,
                op.ret,
                op.last_insert_id_hack,
            ),
            Operation::Transaction(op) => {
                let sql = sql::Serializer::mysql(&schema.db).serialize_transaction(&op);
                self.conn.query_drop(sql).await.map_err(|e| match e {
                    mysql_async::Error::Server(se) => match se.code {
                        1213 => toasty_core::Error::serialization_failure(se.message),
                        1792 => toasty_core::Error::read_only_transaction(se.message),
                        _ => toasty_core::Error::driver_operation_failed(
                            mysql_async::Error::Server(se),
                        ),
                    },
                    other => toasty_core::Error::driver_operation_failed(other),
                })?;
                return Ok(ExecResponse::count(0));
            }
            op => todo!("op={:#?}", op),
        };

        let (sql_as_str, arg_order) =
            sql::Serializer::mysql(&schema.db).serialize_with_arg_order(&sql);

        tracing::debug!(db.system = "mysql", db.statement = %sql_as_str, params = typed_params.len(), "executing SQL");

        // MySQL uses positional `?` without indices, so params must be reordered
        // to match the order `Expr::Arg(n)` placeholders appear in the SQL.
        let params: Vec<_> = arg_order
            .iter()
            .map(|&pos| Value::from(typed_params[pos].value.clone()))
            .collect();
        let args = params
            .iter()
            .map(|param| param.to_value())
            .collect::<Vec<_>>();

        let statement = self
            .conn
            .prep(&sql_as_str)
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        if ret.is_none() {
            let count = self
                .conn
                .exec_iter(&statement, mysql_async::Params::Positional(args))
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?
                .affected_rows();

            // Handle the last_insert_id_hack for MySQL INSERT with RETURNING
            if let Some(num_rows) = last_insert_id_hack {
                // Assert the previous statement was an INSERT
                assert!(
                    matches!(sql, sql::Statement::Insert(_)),
                    "last_insert_id_hack should only be used with INSERT statements"
                );

                // Execute SELECT LAST_INSERT_ID() on the same connection
                let first_id: u64 = self
                    .conn
                    .query_first("SELECT LAST_INSERT_ID()")
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?
                    .ok_or_else(|| {
                        toasty_core::Error::driver_operation_failed(std::io::Error::other(
                            "LAST_INSERT_ID() returned no rows",
                        ))
                    })?;

                // Generate rows with sequential IDs
                let results = (0..num_rows).map(move |offset| {
                    let id = first_id + offset;
                    // Return a record with a single field containing the ID
                    Ok(ValueRecord::from_vec(vec![stmt::Value::U64(id)]))
                });

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
            .map_err(toasty_core::Error::driver_operation_failed)?;

        if let Some(returning) = ret {
            let results = rows.into_iter().map(move |mut row| {
                assert_eq!(
                    row.len(),
                    returning.len(),
                    "row={row:#?}; returning={returning:#?}"
                );

                let mut results = Vec::new();
                for i in 0..row.len() {
                    let column = &row.columns()[i];
                    results.push(Value::from_sql(i, &mut row, column, &returning[i]).into_inner());
                }

                Ok(ValueRecord::from_vec(results))
            });

            Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
                results,
            )))
        } else {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<i64, usize>(0).unwrap();
            let condition_matched = row.get::<i64, usize>(1).unwrap();

            if total == condition_matched {
                Ok(ExecResponse::count(total as _))
            } else {
                Err(toasty_core::Error::condition_failed(
                    "update condition did not match",
                ))
            }
        }
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
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Query all applied migrations
        let rows: Vec<u64> = self
            .conn
            .exec("SELECT id FROM __toasty_migrations ORDER BY applied_at", ())
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

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
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Start transaction
        let mut transaction = self
            .conn
            .start_transaction(Default::default())
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Execute each migration statement
        for statement in migration.statements() {
            if let Err(e) = transaction
                .query_drop(statement)
                .await
                .map_err(toasty_core::Error::driver_operation_failed)
            {
                transaction
                    .rollback()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;
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
            .map_err(toasty_core::Error::driver_operation_failed)
        {
            transaction
                .rollback()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
            return Err(e);
        }

        // Commit transaction
        transaction
            .commit()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        Ok(())
    }
}
