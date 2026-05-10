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

    /// Boolean: `lhs` (array) contains every element of `rhs` (array).
    /// PostgreSQL: `lhs @> rhs`. Drives [`Path::is_superset`](super::Path).
    IsSuperset {
        /// The array claimed to be the superset.
        lhs: Box<Expr>,
        /// The array claimed to be the subset.
        rhs: Box<Expr>,
    },

    /// Boolean: `lhs` and `rhs` (both arrays) share at least one element.
    /// PostgreSQL: `lhs && rhs`. Drives [`Path::intersects`](super::Path).
    Intersects {
        /// The first array operand.
        lhs: Box<Expr>,
        /// The second array operand.
        rhs: Box<Expr>,
    },

    /// Integer: number of elements in an array. PostgreSQL: `cardinality(expr)`.
    /// Drives [`Path::len`](super::Path) and [`Path::is_empty`](super::Path).
    Length {
        /// The array whose length is being measured.
        expr: Box<Expr>,
    },
}

impl From<ExprFunc> for Expr {
    fn from(value: ExprFunc) -> Self {
        Self::Func(value)
    }
}

impl Expr {
    /// Build an `IsSuperset` array predicate (`lhs @> rhs` on PostgreSQL).
    pub fn array_is_superset(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Self::Func(ExprFunc::IsSuperset {
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        })
    }

    /// Build an `Intersects` array predicate (`lhs && rhs` on PostgreSQL).
    pub fn array_intersects(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Self::Func(ExprFunc::Intersects {
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        })
    }

    /// Build an array-length expression (`cardinality(expr)` on PostgreSQL).
    pub fn array_length(expr: impl Into<Self>) -> Self {
        Self::Func(ExprFunc::Length {
            expr: Box::new(expr.into()),
        })
    }
}
