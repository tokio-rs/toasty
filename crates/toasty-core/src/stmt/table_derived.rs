use crate::stmt::Query;

/// A derived table (inline subquery) used as a table reference.
///
/// Wraps a [`Query`] whose result set can be referenced like a table in the
/// outer query's `FROM` clause.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{TableDerived, Query};
///
/// let derived = TableDerived {
///     subquery: Box::new(Query::unit()),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TableDerived {
    /// The subquery whose result set forms this derived table.
    pub subquery: Box<Query>,
}
