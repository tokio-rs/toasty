use super::*;

/// Tests if the string expression matches `pattern`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprLike<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub pattern: Box<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn like(expr: impl Into<Expr<'stmt>>, pattern: impl Into<Expr<'stmt>>) -> Expr<'stmt> {
        ExprLike {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl<'stmt> From<ExprLike<'stmt>> for Expr<'stmt> {
    fn from(value: ExprLike<'stmt>) -> Self {
        Expr::Pattern(value.into())
    }
}

impl<'stmt> From<ExprLike<'stmt>> for ExprPattern<'stmt> {
    fn from(value: ExprLike<'stmt>) -> Self {
        ExprPattern::Like(value)
    }
}
