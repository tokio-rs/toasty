#![warn(missing_docs)]

//! Toasty driver for [Cloudflare D1](https://developers.cloudflare.com/d1/).
//!
//! The driver runs inside a Cloudflare Worker built for
//! `wasm32-unknown-unknown`. Give it the D1 binding from the Worker's
//! environment:
//!
//! ```ignore
//! use toasty_driver_d1::D1;
//!
//! #[worker::event(fetch)]
//! async fn fetch(
//!     _req: worker::Request,
//!     env: worker::Env,
//!     _ctx: worker::Context,
//! ) -> worker::Result<worker::Response> {
//!     let db = toasty::Db::builder()
//!         .models(toasty::models!(crate::*))
//!         .build(D1::new(env.d1("DB")?))
//!         .await
//!         .map_err(|e| worker::Error::RustError(e.to_string()))?;
//!     // ...
//! #   todo!()
//! }
//! ```
//!
//! # Limitations
//!
//! - D1 rejects `BEGIN`, `COMMIT`, and `SAVEPOINT`. `Db::transaction`
//!   fails with an unsupported-feature error, and a statement that Toasty
//!   runs as several queries (for example, a create with nested
//!   associations) is not atomic.
//! - D1 moves integers as JS numbers and does not accept `BigInt`. Binding
//!   or reading an `i64` or `u64` larger than `2^53 - 1` in magnitude
//!   fails, because the number cannot hold it exactly.
//! - D1 accepts at most 100 bound parameters per query. A statement that
//!   needs more (for example, inserting many rows at once) fails.
//! - The driver does not emit the `toasty::query` event, because
//!   `std::time::Instant` is not available on `wasm32-unknown-unknown`.

mod value;

use async_trait::async_trait;
use std::{borrow::Cow, sync::Arc};
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, ConnectContext, Driver, ExecResponse,
        operation::{Operation, RawSqlRet, TypedValue},
    },
    schema::{
        db::{AppliedMigration, Migration},
        diff,
    },
    stmt,
};
use toasty_sql as sql;
use worker::{D1Database, js_sys::Array, send::SendFuture};

enum SqlReturn {
    Count,
    Infer,
    Types(Vec<stmt::Type>),
}

const CREATE_MIGRATIONS_TABLE: &str = "CREATE TABLE IF NOT EXISTS __toasty_migrations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
)";

/// A [`Driver`] for a Cloudflare D1 database binding.
#[derive(Debug)]
pub struct D1 {
    db: Arc<D1Database>,
}

impl D1 {
    /// Create a driver for the D1 database bound to the Worker, as returned
    /// by `worker::Env::d1`.
    pub fn new(db: D1Database) -> Self {
        Self { db: Arc::new(db) }
    }
}

#[async_trait]
impl Driver for D1 {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed("d1:")
    }

    fn capability(&self) -> &'static Capability {
        &Capability::D1
    }

    async fn connect(&self, _cx: &ConnectContext) -> Result<Box<dyn toasty_core::Connection>> {
        Ok(Box::new(Connection {
            db: self.db.clone(),
        }))
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::D1);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| sql::Serializer::sqlite(stmt.schema()).serialize(stmt.statement()))
            .collect();

        Migration::new_sql_with_breakpoints(&sql_strings)
    }

    async fn reset_db(&self) -> Result<()> {
        Err(toasty_core::Error::unsupported_feature(
            "D1 cannot reset its database; recreate it with `wrangler d1`",
        ))
    }
}

/// A connection to a D1 database.
///
/// D1 has no connection state; every connection shares the binding.
#[derive(Debug)]
pub struct Connection {
    db: Arc<D1Database>,
}

// D1 futures hold JS values, so they are not `Send`. A Worker runs on one
// thread, so `SendFuture` lets them satisfy the `Send` bound of the driver
// traits without moving between threads.
#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        SendFuture::new(self.exec_op(schema, op)).await
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        let serializer = sql::Serializer::sqlite(&schema.db);
        let mut statements = vec![];

        for table in &schema.db.tables {
            tracing::debug!(table = %table.name, "creating table");
            statements
                .push(serializer.serialize(&sql::Statement::create_table(table, &Capability::D1)));

            // The PK is created by the table statement
            for index in table.indices.iter().filter(|index| !index.primary_key) {
                statements.push(serializer.serialize(&sql::Statement::create_index(index)));
            }
        }

        SendFuture::new(self.batch(statements)).await
    }

    async fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>> {
        SendFuture::new(async {
            self.batch(vec![CREATE_MIGRATIONS_TABLE.to_owned()]).await?;

            // Migration IDs use all 63 bits, more than a JS number holds, so
            // they cross the binding as text.
            let rows: Vec<Vec<String>> = self
                .db
                .prepare("SELECT CAST(id AS TEXT) FROM __toasty_migrations ORDER BY applied_at")
                .raw()
                .await
                .map_err(driver_error)?;

            rows.into_iter()
                .map(|row| {
                    let id = row[0]
                        .parse()
                        .map_err(toasty_core::Error::driver_operation_failed)?;
                    Ok(AppliedMigration::new(id))
                })
                .collect()
        })
        .await
    }

    async fn apply_migration(&mut self, id: u64, name: &str, migration: &Migration) -> Result<()> {
        tracing::info!(id = id, name = %name, "applying migration");

        SendFuture::new(async {
            self.batch(vec![CREATE_MIGRATIONS_TABLE.to_owned()]).await?;

            // D1 runs a batch as one transaction, so the migration and its
            // record apply together or not at all.
            let mut statements = migration
                .statements()
                .into_iter()
                .map(|sql| self.db.prepare(sql))
                .collect::<Vec<_>>();
            statements.push(
                self.db
                    .prepare(
                        "INSERT INTO __toasty_migrations (id, name, applied_at) \
                         VALUES (CAST(?1 AS INTEGER), ?2, datetime('now'))",
                    )
                    .bind(&[id.to_string().into(), name.into()])
                    .map_err(driver_error)?,
            );

            self.db.batch(statements).await.map_err(driver_error)?;
            Ok(())
        })
        .await
    }
}

impl Connection {
    async fn exec_op(&self, schema: &Schema, op: Operation) -> Result<ExecResponse> {
        tracing::trace!(driver = "d1", op = %op.name(), "driver exec");

        let (sql, typed_params, ret_tys) = match op {
            Operation::Insert(op) => (sql::Statement::from(op.stmt), op.params, op.ret),
            Operation::QuerySql(op) => (sql::Statement::from(op.stmt), op.params, op.ret),
            Operation::RawSql(op) => {
                let ret = match op.ret {
                    RawSqlRet::None => SqlReturn::Count,
                    RawSqlRet::Infer => SqlReturn::Infer,
                    RawSqlRet::Types(types) => SqlReturn::Types(types),
                };
                return self.query(&op.sql, op.params, ret).await;
            }
            Operation::Transaction(_) => {
                return Err(toasty_core::Error::unsupported_feature(
                    "D1 does not support transactions",
                ));
            }
            _ => {
                return Err(toasty_core::Error::unsupported_feature(format!(
                    "D1 does not support the {} operation",
                    op.name()
                )));
            }
        };

        let returning = match &sql {
            sql::Statement::Query(_) => true,
            sql::Statement::Insert(stmt) => stmt.returning.is_some(),
            sql::Statement::Delete(stmt) => stmt.returning.is_some(),
            sql::Statement::Update(stmt) => stmt.returning.is_some(),
            _ => false,
        };
        let ret = if returning {
            SqlReturn::Types(ret_tys.unwrap())
        } else {
            SqlReturn::Count
        };

        let sql_str = sql::Serializer::sqlite(&schema.db).serialize(&sql);
        self.query(&sql_str, typed_params, ret).await
    }

    async fn query(
        &self,
        sql_str: &str,
        typed_params: Vec<TypedValue>,
        ret: SqlReturn,
    ) -> Result<ExecResponse> {
        let params = typed_params
            .iter()
            .map(|tv| value::to_js(&tv.value))
            .collect::<Result<Vec<_>>>()?;
        let stmt = self
            .db
            .prepare(sql_str)
            .bind(&params)
            .map_err(driver_error)?;

        let types = match ret {
            SqlReturn::Count => {
                let meta = stmt
                    .run()
                    .await
                    .and_then(|result| result.meta())
                    .map_err(driver_error)?;
                let changes = meta.and_then(|meta| meta.changes).unwrap_or(0);
                return Ok(ExecResponse::count(changes as u64));
            }
            SqlReturn::Infer => None,
            SqlReturn::Types(types) => Some(types),
        };

        let rows = stmt
            .raw_js_value()
            .await
            .map_err(driver_error)?
            .iter()
            .map(|row| {
                let row = Array::from(row);
                let items = match &types {
                    Some(types) => types
                        .iter()
                        .enumerate()
                        .map(|(index, ty)| value::from_js(row.get(index as u32), ty))
                        .collect::<Result<_>>()?,
                    None => row.iter().map(value::from_js_infer).collect(),
                };
                Ok(stmt::ValueRecord::from_vec(items).into())
            })
            .collect::<Result<_>>()?;

        Ok(ExecResponse::value_stream(stmt::ValueStream::from_vec(
            rows,
        )))
    }

    /// Runs `statements` as one D1 batch, which D1 applies atomically.
    async fn batch(&self, statements: Vec<String>) -> Result<()> {
        let statements = statements
            .into_iter()
            .map(|sql| self.db.prepare(sql))
            .collect();
        self.db.batch(statements).await.map_err(driver_error)?;
        Ok(())
    }
}

/// Wraps a [`worker::Error`], which holds JS values and is not `Send`.
fn driver_error(err: worker::Error) -> toasty_core::Error {
    let message = match err {
        // The D1 message without the JS stack trace.
        worker::Error::D1(err) => err.cause(),
        err => err.to_string(),
    };
    toasty_core::Error::driver_operation_failed(std::io::Error::other(message))
}
