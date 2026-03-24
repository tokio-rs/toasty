use super::Cte;

/// A `WITH` clause containing one or more common table expressions (CTEs).
///
/// CTEs are temporary named result sets defined at the beginning of a query,
/// referenced by [`TableRef::Cte`](super::TableRef) in the query body.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{With, Cte, Query};
///
/// let with = With {
///     ctes: vec![Cte { query: Query::unit() }],
/// };
/// assert_eq!(with.ctes.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct With {
    /// The list of CTEs, referenced by index from [`TableRef::Cte`](super::TableRef).
    pub ctes: Vec<Cte>,
}

impl From<Vec<Cte>> for With {
    fn from(ctes: Vec<Cte>) -> Self {
        Self { ctes }
    }
}
