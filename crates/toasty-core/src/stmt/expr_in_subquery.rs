use crate::stmt::{Node, Visit, VisitMut};

use super::{Expr, Query};

/// Tests whether a value is in the results of a subquery.
///
/// Returns `true` if `expr` matches any row returned by `query`, or the
/// opposite when `negated` is set. The flag makes `NOT IN` a predicate in
/// its own right rather than the negation of `IN`: statement normalization
/// attaches variant guards to the predicate before expanding the negated
/// form to `not(in_subquery(..))`, so the guards end up outside the
/// negation.
///
/// # Examples
///
/// ```text
/// in_subquery(x, select(...))      // returns `true` if `x` is in the subquery results
/// not_in_subquery(x, select(...))  // returns `true` if `x` is not in the subquery results
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprInSubquery {
    /// The value to search for.
    pub expr: Box<Expr>,

    /// The subquery to search within.
    pub query: Box<Query>,

    /// Whether the check is `NOT IN`.
    pub negated: bool,
}

impl Expr {
    /// Creates an `IN (subquery)` expression: `lhs IN (SELECT ...)`.
    pub fn in_subquery(lhs: impl Into<Self>, rhs: impl Into<Query>) -> Self {
        ExprInSubquery {
            expr: Box::new(lhs.into()),
            query: Box::new(rhs.into()),
            negated: false,
        }
        .into()
    }

    /// Creates a `NOT IN (subquery)` expression: `lhs NOT IN (SELECT ...)`.
    pub fn not_in_subquery(lhs: impl Into<Self>, rhs: impl Into<Query>) -> Self {
        ExprInSubquery {
            expr: Box::new(lhs.into()),
            query: Box::new(rhs.into()),
            negated: true,
        }
        .into()
    }

    /// Returns `true` if this expression is an `IN (subquery)` check.
    pub fn is_in_subquery(&self) -> bool {
        matches!(self, Self::InSubquery(_))
    }
}

impl Node for ExprInSubquery {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_expr_in_subquery(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_expr_in_subquery_mut(self);
    }
}

impl From<ExprInSubquery> for Expr {
    fn from(value: ExprInSubquery) -> Self {
        Self::InSubquery(value)
    }
}
