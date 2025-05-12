use super::{Expr, Offset};

#[derive(Debug, Clone)]
pub struct Limit {
    /// The limit expression
    pub limit: Expr,

    /// The offset expression
    pub offset: Option<Offset>,
}
