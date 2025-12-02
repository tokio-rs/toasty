use crate::stmt::{Node, Visit, VisitMut};

use super::{Expr, Query};

/// Tests whether a value is in the results of a subquery.
///
/// Returns `true` if `expr` matches any row returned by `query`.
///
/// # Examples
///
/// ```text
/// in_subquery(x, select(...))  // returns `true` if `x` is in the subquery results
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprInSubquery {
    /// The value to search for.
    pub expr: Box<Expr>,

    /// The subquery to search within.
    pub query: Box<Query>,
}

impl Expr {
    pub fn in_subquery(lhs: impl Into<Self>, rhs: impl Into<Query>) -> Self {
        ExprInSubquery {
            expr: Box::new(lhs.into()),
            query: Box::new(rhs.into()),
        }
        .into()
    }

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
