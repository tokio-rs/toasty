use super::OrderByExpr;

/// An `ORDER BY` clause containing one or more ordering expressions.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{OrderBy, OrderByExpr, Direction, Expr};
///
/// let order = OrderBy {
///     exprs: vec![OrderByExpr {
///         expr: Expr::null(),
///         order: Some(Direction::Asc),
///     }],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    /// The list of ordering expressions, applied in order.
    pub exprs: Vec<OrderByExpr>,
}

impl OrderBy {
    /// Flips the direction of each [`OrderByExpr`] that makes up this [`OrderBy`].
    pub fn reverse(&mut self) {
        for expr in &mut self.exprs {
            expr.reverse();
        }
    }
}

impl From<OrderByExpr> for OrderBy {
    fn from(value: OrderByExpr) -> Self {
        Self { exprs: vec![value] }
    }
}
