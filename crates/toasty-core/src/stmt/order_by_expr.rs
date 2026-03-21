use super::{Direction, Expr};

/// A single expression within an [`OrderBy`](super::OrderBy) clause, with an
/// optional sort direction.
///
/// When `order` is `None`, the database default direction is used (typically
/// ascending).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{OrderByExpr, Direction, Expr};
///
/// let expr = OrderByExpr {
///     expr: Expr::null(),
///     order: Some(Direction::Desc),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByExpr {
    /// The expression to order by.
    pub expr: Expr,

    /// The sort direction, or `None` for the database default.
    pub order: Option<Direction>,
}

impl OrderByExpr {
    /// Flips the sort direction. `Desc` becomes default (ascending); default
    /// and `Asc` become `Asc` (explicit ascending, since the default changed).
    pub fn reverse(&mut self) {
        self.order = match self.order {
            Some(Direction::Desc) => None,
            _ => Some(Direction::Asc),
        }
    }
}
