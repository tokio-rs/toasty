#![allow(clippy::needless_range_loop)]

mod value;
pub(crate) use value::Value;

use mysql_async::{
    prelude::{Queryable, ToValue},
    Conn, Pool,
};
use std::{borrow::Cow, sync::Arc};
use toasty_core::{
    async_trait,
    driver::{operation::Transaction, Capability, Driver, Operation, Response},
    schema::db::{Migration, Schema, SchemaDiff, Table},
    stmt::{self, ValueRecord},
    Result,
};
use toasty_sql::{self as sql, TypedValue};
use url::Url;

#[derive(Debug)]
pub struct MySQL {
    url: String,
    pool: Pool,
}

impl MySQL {
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

        let pool = Pool::new(opts);
        Ok(Self { url: url_str, pool })
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
        let conn = self
            .pool
            .get_conn()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        Ok(Box::new(Connection::new(conn)))
    }

    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::MYSQL);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| {
                let mut params = Vec::<TypedValue>::new();
                let sql =
                    sql::Serializer::mysql(stmt.schema()).serialize(stmt.statement(), &mut params);
                assert!(
                    params.is_empty(),
                    "migration statements should not have parameters"
                );
                sql
            })
            .collect();

        Migration::new_sql_with_breakpoints(&sql_strings)
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        let mut conn = self
            .pool
            .get_conn()
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

#[derive(Debug)]
pub struct Connection {
    conn: Conn,
}

impl Connection {
    pub fn new(conn: Conn) -> Self {
        Self { conn }
    }

    pub async fn create_table(&mut self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);

        let mut params: Vec<toasty_sql::TypedValue> = Vec::new();

        let sql = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::MYSQL),
            &mut params,
        );

        assert!(
            params.is_empty(),
            "creating a table shouldn't involve any parameters"
        );

        self.conn
            .exec_drop(&sql, ())
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index), &mut params);

            assert!(
                params.is_empty(),
                "creating an index shouldn't involve any parameters"
            );

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
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let (sql, ret, last_insert_id_hack): (sql::Statement, _, _) = match op {
            Operation::QuerySql(op) => (op.stmt.into(), op.ret, op.last_insert_id_hack),
            Operation::Transaction(Transaction::Start) => {
                self.conn
                    .query_drop("START TRANSACTION")
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Commit) => {
                self.conn
                    .query_drop("COMMIT")
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Rollback) => {
                self.conn
                    .query_drop("ROLLBACK")
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(Response::count(0));
            }
            op => todo!("op={:#?}", op),
        };

        let mut params: Vec<toasty_sql::TypedValue> = Vec::new();

        let sql_as_str = sql::Serializer::mysql(schema).serialize(&sql, &mut params);

        let params = params
            .into_iter()
            .map(|tv| Value::from(tv.value))
            .collect::<Vec<_>>();
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

                return Ok(Response::value_stream(stmt::ValueStream::from_iter(
                    results,
                )));
            }

            return Ok(Response::count(count));
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

            Ok(Response::value_stream(stmt::ValueStream::from_iter(
                results,
            )))
        } else {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<i64, usize>(0).unwrap();
            let condition_matched = row.get::<i64, usize>(1).unwrap();

            if total == condition_matched {
                Ok(Response::count(total as _))
            } else {
                Err(toasty_core::Error::condition_failed(
                    "update condition did not match",
                ))
            }
        }
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            self.create_table(schema, table).await?;
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
        name: String,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
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
