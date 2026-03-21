use super::{Expr, ExprSet, Query};

/// A `VALUES` clause: a set of row expressions.
///
/// Used as the source for `INSERT` statements and as a query body type that
/// produces literal rows without reading from a table.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Values, Expr};
///
/// let values = Values::new(vec![Expr::null()]);
/// assert_eq!(values.rows.len(), 1);
/// assert!(!values.is_empty());
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Values {
    /// The row expressions. Each element is one row to insert or return.
    pub rows: Vec<Expr>,
}

impl Values {
    /// Creates a `Values` from a vector of row expressions.
    pub fn new(rows: Vec<Expr>) -> Self {
        Self { rows }
    }

    /// Returns `true` if there are no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns `true` if all rows are constant expressions.
    pub fn is_const(&self) -> bool {
        self.rows.iter().all(|row| row.is_const())
    }
}

impl From<Values> for ExprSet {
    fn from(value: Values) -> Self {
        Self::Values(value)
    }
}

impl From<Values> for Query {
    fn from(value: Values) -> Self {
        Self::builder(value).build()
    }
}

impl From<Expr> for Values {
    fn from(value: Expr) -> Self {
        Self { rows: vec![value] }
    }
}
