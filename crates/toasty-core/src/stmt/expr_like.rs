use super::*;

/// Tests if the string expression matches `pattern`.
#[derive(Debug, Clone)]
pub struct ExprLike {
    pub expr: Box<Expr>,
    pub pattern: Box<Expr>,
}

impl Expr {
    pub fn like(expr: impl Into<Self>, pattern: impl Into<Self>) -> Self {
        ExprLike {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl From<ExprLike> for Expr {
    fn from(value: ExprLike) -> Self {
        Self::Pattern(value.into())
    }
}

impl From<ExprLike> for ExprPattern {
    fn from(value: ExprLike) -> Self {
        Self::Like(value)
    }
}
