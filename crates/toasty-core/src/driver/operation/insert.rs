use super::{Operation, TypedValue};

use crate::stmt;

/// Inserts one or more records into a table.
///
/// Contains a lowered [`stmt::Statement`] (always an insert statement) and an
/// optional return type describing the columns the driver should return after
/// the insert (e.g., auto-generated keys).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{Insert, Operation};
///
/// let op = Insert {
///     stmt: insert_statement,
///     params: vec![],
///     ret: Some(vec![stmt::Type::I64]),
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct Insert {
    /// The insert statement to execute. Scalar values that should be sent as
    /// bind parameters have been replaced with `Expr::Arg(n)` where `n` is
    /// the index into [`params`](Self::params).
    pub stmt: stmt::Statement,

    /// Typed bind parameters extracted from the statement.
    pub params: Vec<TypedValue>,

    /// The types of values the insert must return. SQL backends with native
    /// mutation `RETURNING` decode its projected rows. A backend may also
    /// provide an exact operation-specific result, such as MySQL's generated
    /// ID for a single-row insert. When `None`, no rows are returned.
    pub ret: Option<Vec<stmt::Type>>,
}

impl From<Insert> for Operation {
    fn from(value: Insert) -> Self {
        Self::Insert(value)
    }
}
