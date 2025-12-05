use super::{Direction, Expr};

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByExpr {
    /// The expression
    pub expr: Expr,

    /// Ascending or descending
    pub order: Option<Direction>,
}

impl OrderByExpr {
    /// Flips the direction by which the query is ordered.
    pub fn reverse(&mut self) {
        self.order = match self.order {
            Some(Direction::Desc) => None,
            _ => Some(Direction::Asc),
        }
    }
}
