use super::Query;

/// A common table expression (CTE) within a [`With`](super::With) clause.
///
/// Each CTE wraps a [`Query`] whose result set can be referenced as a named
/// table in the outer query via [`TableRef::Cte`](super::TableRef).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Cte, Query};
///
/// let cte = Cte { query: Query::unit() };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    /// The query defining this CTE's result set.
    pub query: Query,
}
