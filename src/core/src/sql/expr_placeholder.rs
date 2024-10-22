use super::*;

#[derive(Debug, Clone)]
pub struct ExprPlaceholder {
    pub position: usize,
}

impl<'stmt> From<ExprPlaceholder> for Expr<'stmt> {
    fn from(value: ExprPlaceholder) -> Self {
        Expr::Placeholder(value)
    }
}
