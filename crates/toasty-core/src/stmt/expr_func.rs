use super::{Expr, FuncCount, FuncLastInsertId};

/// A function call expression.
///
/// Represents aggregate or scalar functions applied to expressions.
///
/// # Examples
///
/// ```text
/// count(*)           // counts all rows
/// count(field)       // counts non-null values
/// last_insert_id()   // MySQL: get the last auto-increment ID
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum ExprFunc {
    /// The `count` aggregate function.
    Count(FuncCount),

    /// The `LAST_INSERT_ID()` function (MySQL-specific).
    ///
    /// Returns the first auto-increment ID that was generated for an INSERT statement.
    /// When multiple rows are inserted, this returns the ID of the first row.
    LastInsertId(FuncLastInsertId),
}

impl From<ExprFunc> for Expr {
    fn from(value: ExprFunc) -> Self {
        Self::Func(value)
    }
}
