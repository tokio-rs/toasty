//! Raw SQL execution helpers for SQL backends.
//!
//! Use [`statement`] for SQL that returns an affected-row count and [`query`]
//! for SQL that returns rows. Both builders execute through
//! [`Executor`](crate::Executor), so they work with [`Db`](crate::Db),
//! [`Connection`](crate::Connection), and [`Transaction`](crate::Transaction).
//!
//! Raw SQL is backend SQL. Toasty does not rewrite placeholders, quote
//! identifiers, or hydrate models from raw query results. Use the placeholder
//! syntax of the active driver. The syntax is exposed at runtime through
//! [`Capability::sql_placeholder`](crate::Capability::sql_placeholder):
//!
//! | Backend | [`SqlPlaceholder`](crate::SqlPlaceholder) |
//! |---|---|
//! | SQLite | `NumberedQuestionMark` (`?1`, `?2`, ...) |
//! | Turso | `NumberedQuestionMark` (`?1`, `?2`, ...) |
//! | PostgreSQL | `DollarNumber` (`$1`, `$2`, ...) |
//! | MySQL | `QuestionMark` (`?`, `?`, ...) |
//!
//! # Statements
//!
//! ```ignore
//! let updated = toasty::sql::statement(
//!     "UPDATE users SET name = ?1 WHERE id = ?2",
//! )
//! .bind("Alice")
//! .bind(1_i64)
//! .exec(&mut db)
//! .await?;
//!
//! assert_eq!(updated, 1);
//! ```
//!
//! # Queries
//!
//! Queries return dynamic [`Value`] rows. Each row is a [`Value::Record`] with
//! fields in selected-column order.
//!
//! ```ignore
//! let rows = toasty::sql::query(
//!     "SELECT id, name FROM users WHERE active = ?1",
//! )
//! .bind(true)
//! .exec(&mut db)
//! .await?;
//! ```
//!
//! The driver infers result value types from database metadata by default. Use
//! [`Query::column_types`] when a result column is ambiguous, such as a SQLite
//! boolean stored as an integer or a UUID stored as bytes.

use crate::{Executor, Result};
use toasty_core::{
    Error,
    driver::{
        Capability,
        operation::{RawSql, RawSqlRet, TypedValue},
    },
    schema::db,
    stmt::{self, Value},
};

/// A raw SQL statement that returns an affected-row count.
///
/// Create one with [`statement`].
pub struct Statement {
    sql: String,
    params: Vec<Param>,
}

/// A raw SQL query that returns dynamic rows.
///
/// Create one with [`query`].
pub struct Query {
    sql: String,
    params: Vec<Param>,
    ret: RawSqlRet,
}

#[derive(Debug, Clone)]
enum Param {
    Infer(Value),
    Typed(TypedValue),
}

/// Create a raw SQL statement.
///
/// Statements are executed for their affected-row count. Use
/// [`query`] for SQL that returns rows.
///
/// The SQL string must use the placeholder syntax reported by
/// [`Capability::sql_placeholder`](crate::Capability::sql_placeholder).
///
/// ```ignore
/// let count = toasty::sql::statement(
///     "DELETE FROM users WHERE archived = ?1",
/// )
/// .bind(true)
/// .exec(&mut db)
/// .await?;
/// ```
pub fn statement(sql: impl Into<String>) -> Statement {
    Statement {
        sql: sql.into(),
        params: vec![],
    }
}

/// Create a raw SQL query.
///
/// Query rows are returned as dynamic [`Value`] records.
///
/// ```ignore
/// let rows = toasty::sql::query(
///     "SELECT id, name FROM users WHERE active = ?1",
/// )
/// .bind(true)
/// .exec(&mut db)
/// .await?;
/// ```
pub fn query(sql: impl Into<String>) -> Query {
    Query {
        sql: sql.into(),
        params: vec![],
        ret: RawSqlRet::Infer,
    }
}

impl Statement {
    /// Bind a value using the executor driver's default database type for
    /// that value.
    ///
    /// Values are bound in the order this method is called. Use
    /// [`bind_typed`](Self::bind_typed) for `NULL`, empty lists, or values
    /// whose database type cannot be inferred from the value alone.
    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.params.push(Param::Infer(value.into()));
        self
    }

    /// Bind a value with an explicit database type.
    ///
    /// This is useful for `NULL`, empty lists, or backend-specific column
    /// types.
    ///
    /// ```ignore
    /// use toasty::schema::db;
    ///
    /// toasty::sql::statement(
    ///     "UPDATE users SET archived_at = ?1 WHERE id = ?2",
    /// )
    /// .bind_typed(toasty::stmt::Value::Null, db::Type::Timestamp(6))
    /// .bind(1_i64)
    /// .exec(&mut db)
    /// .await?;
    /// ```
    pub fn bind_typed(mut self, value: impl Into<Value>, ty: db::Type) -> Self {
        self.params.push(Param::Typed(TypedValue {
            value: value.into(),
            ty,
        }));
        self
    }

    /// Execute the statement and return the affected-row count.
    ///
    /// The executor may be a [`Db`](crate::Db), [`Connection`](crate::Connection),
    /// or [`Transaction`](crate::Transaction).
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<u64> {
        let raw = into_raw_sql(
            self.sql,
            self.params,
            RawSqlRet::None,
            executor.capability(),
        )?;
        let response = executor.exec_raw_sql(raw).await?;

        Ok(response.values.into_count())
    }
}

impl Query {
    /// Bind a value using the executor driver's default database type for
    /// that value.
    ///
    /// Values are bound in the order this method is called. Use
    /// [`bind_typed`](Self::bind_typed) for `NULL`, empty lists, or values
    /// whose database type cannot be inferred from the value alone.
    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.params.push(Param::Infer(value.into()));
        self
    }

    /// Bind a value with an explicit database type.
    ///
    /// This is useful for `NULL`, empty lists, or backend-specific column
    /// types.
    pub fn bind_typed(mut self, value: impl Into<Value>, ty: db::Type) -> Self {
        self.params.push(Param::Typed(TypedValue {
            value: value.into(),
            ty,
        }));
        self
    }

    /// Provide result column type hints for ambiguous backend values.
    ///
    /// Hints affect result decoding only. They do not change the SQL sent to
    /// the database. Use this when backend metadata is not enough to choose the
    /// desired Toasty value type.
    ///
    /// ```ignore
    /// use toasty::stmt;
    ///
    /// let rows = toasty::sql::query(
    ///     "SELECT id, enabled FROM users WHERE id = ?1",
    /// )
    /// .bind(1_i64)
    /// .column_types([stmt::Type::I64, stmt::Type::Bool])
    /// .exec(&mut db)
    /// .await?;
    /// ```
    pub fn column_types(mut self, types: impl IntoIterator<Item = stmt::Type>) -> Self {
        self.ret = RawSqlRet::Types(types.into_iter().collect());
        self
    }

    /// Execute the query and return dynamic rows.
    ///
    /// Each row is a [`Value::Record`] with fields in selected-column order.
    /// The executor may be a [`Db`](crate::Db), [`Connection`](crate::Connection),
    /// or [`Transaction`](crate::Transaction).
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<Vec<Value>> {
        let raw = into_raw_sql(self.sql, self.params, self.ret, executor.capability())?;
        let response = executor.exec_raw_sql(raw).await?;

        match response.values.collect_as_value().await? {
            Value::List(items) => Ok(items),
            value => Err(toasty_core::Error::invalid_result(format!(
                "raw SQL query expected a list of rows, got {value:?}"
            ))),
        }
    }
}

fn into_raw_sql(
    sql: String,
    params: Vec<Param>,
    ret: RawSqlRet,
    capability: &Capability,
) -> Result<RawSql> {
    if !capability.sql {
        return Err(Error::unsupported_feature(format!(
            "{} does not support raw SQL",
            capability.driver_name
        )));
    }

    Ok(RawSql {
        sql,
        params: params
            .into_iter()
            .map(|param| param.into_typed(capability))
            .collect::<Result<_>>()?,
        ret,
    })
}

impl Param {
    fn into_typed(self, capability: &Capability) -> Result<TypedValue> {
        match self {
            Param::Infer(value) => infer_typed_value(value, capability),
            Param::Typed(value) => Ok(value),
        }
    }
}

fn infer_typed_value(value: Value, capability: &Capability) -> Result<TypedValue> {
    let ty = value
        .infer_db_ty(&capability.storage_types)
        .map_err(|err| {
            err.context(Error::invalid_statement(
                "cannot infer raw SQL bind type; use bind_typed",
            ))
        })?;

    Ok(TypedValue { value, ty })
}
