use super::{Expr, FuncCount, FuncJsonExtract, FuncLastInsertId, FuncUnnest};

/// A function call expression.
///
/// Represents aggregate, scalar, and set-returning functions applied to
/// expressions.
///
/// # Examples
///
/// ```text
/// count(*)           // counts all rows
/// count(field)       // counts non-null values
/// last_insert_id()   // MySQL: get the last auto-increment ID
/// unnest(array)       // PostgreSQL: expands an array into rows
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

    /// Extracts a value at a key path from a document-stored value (a JSON
    /// extraction on the SQL backends). Produced when filtering on a field
    /// inside a `#[document]` embed.
    JsonExtract(FuncJsonExtract),

    /// Expands a PostgreSQL array into a set of rows.
    Unnest(FuncUnnest),
}

impl From<ExprFunc> for Expr {
    fn from(value: ExprFunc) -> Self {
        Self::Func(value)
    }
}
