use super::{Expr, Offset};

#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    /// The limit expression
    pub limit: Expr,

    /// The offset expression
    pub offset: Option<Offset>,
}
