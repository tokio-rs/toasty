use crate::stmt::{Expr, Query};

/// Tests whether a subquery returns any rows.
///
/// Returns `true` if the subquery produces at least one row.
///
/// # Examples
///
/// ```text
/// exists(subquery)      // returns `true` if subquery has results
/// not_exists(subquery)  // returns `true` if subquery has no results
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprExists {
    /// The subquery to check.
    pub subquery: Box<Query>,
}

impl Expr {
    pub fn exists(subquery: impl Into<Query>) -> Expr {
        Expr::Exists(ExprExists {
            subquery: Box::new(subquery.into()),
        })
    }

    pub fn not_exists(subquery: impl Into<Query>) -> Expr {
        Expr::not(Expr::exists(subquery))
    }
}

impl From<ExprExists> for Expr {
    fn from(value: ExprExists) -> Self {
        Self::Exists(value)
    }
}
