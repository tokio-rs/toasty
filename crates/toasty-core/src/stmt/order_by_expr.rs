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
///     nulls_first: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByExpr {
    /// The expression to order by.
    pub expr: Expr,

    /// The sort direction, or `None` for the database default.
    pub order: Option<Direction>,
    /// Explicit storage null placement, or `None` for the backend default.
    pub nulls_first: Option<bool>,
}

impl OrderByExpr {
    /// Flips the sort direction. `Desc` becomes `Asc`; default (ascending)
    /// and `Asc` become `Desc`.
    pub fn reverse(&mut self) {
        self.nulls_first = self.nulls_first.map(|first| !first);
        self.order = match self.order {
            Some(Direction::Desc) => Some(Direction::Asc),
            _ => Some(Direction::Desc),
        }
    }
}
