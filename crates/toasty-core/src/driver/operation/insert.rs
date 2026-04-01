use super::Operation;

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
///     ret: Some(vec![stmt::Type::I64]),
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct Insert {
    /// The insert statement to execute.
    pub stmt: stmt::Statement,

    /// The types of columns to return from the insert. When `Some`, the driver
    /// should return the inserted row(s) projected to these types (e.g.,
    /// auto-increment IDs). When `None`, no rows are returned.
    pub ret: Option<Vec<stmt::Type>>,

    /// Typed parameter values that were substituted into the statement.
    ///
    /// Each entry pairs a value with its inferred type. Drivers may use this
    /// list instead of re-extracting parameters during SQL serialization.
    pub params: Vec<stmt::Param>,
}

impl From<Insert> for Operation {
    fn from(value: Insert) -> Self {
        Self::Insert(value)
    }
}
