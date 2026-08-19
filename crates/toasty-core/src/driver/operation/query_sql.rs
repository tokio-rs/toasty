use super::{Operation, TypedValue};

use crate::stmt;

/// Executes a SQL statement against the database.
///
/// Only sent to SQL-capable drivers (those with [`Capability::sql`](super::super::Capability)
/// set to `true`). The statement is a fully lowered [`stmt::Statement`] that
/// the SQL serialization layer converts into a backend-specific SQL string.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{QuerySql, Operation};
///
/// let op = QuerySql {
///     stmt: sql_statement,
///     ret: Some(vec![stmt::Type::String, stmt::Type::I64]),
///     last_insert_id: false,
/// };
/// let operation: Operation = op.into();
/// assert!(operation.is_query_sql());
/// ```
#[derive(Debug, Clone)]
pub struct QuerySql {
    /// The SQL statement to execute. Scalar values that should be sent as
    /// bind parameters have been replaced with `Expr::Arg(n)` where `n` is
    /// the index into [`params`](Self::params).
    pub stmt: stmt::Statement,

    /// Typed bind parameters extracted from the statement. Each entry
    /// corresponds to an `Expr::Arg(n)` placeholder in the statement.
    pub params: Vec<TypedValue>,

    /// The types of columns in the result set. When `Some`, the driver uses
    /// these types to decode returned rows. When `None`, the statement does
    /// not return rows (e.g., `DELETE` without `RETURNING`).
    pub ret: Option<Vec<stmt::Type>>,

    /// Report the value the `INSERT` generated instead of a row count.
    ///
    /// Set only for a single-row `INSERT` on a SQL backend without
    /// [`Capability::returning_from_mutation`](super::super::Capability::returning_from_mutation).
    /// The driver answers with one row holding one value: the auto-increment
    /// value the statement generated, which MySQL reports through
    /// `LAST_INSERT_ID()` on the same connection. A driver that has the
    /// capability never sees this set, and asserts as much.
    pub last_insert_id: bool,
}

impl Operation {
    /// Returns `true` if this is a [`QuerySql`](Operation::QuerySql) operation.
    pub fn is_query_sql(&self) -> bool {
        matches!(self, Operation::QuerySql(_))
    }
}

impl From<QuerySql> for Operation {
    fn from(value: QuerySql) -> Self {
        Self::QuerySql(value)
    }
}
