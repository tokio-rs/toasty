use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprBeginsWith<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub pattern: Box<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn begins_with(
        expr: impl Into<Expr<'stmt>>,
        pattern: impl Into<Expr<'stmt>>,
    ) -> Expr<'stmt> {
        ExprBeginsWith {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl<'stmt> From<ExprBeginsWith<'stmt>> for Expr<'stmt> {
    fn from(value: ExprBeginsWith<'stmt>) -> Self {
        Expr::Pattern(value.into())
    }
}

impl<'stmt> From<ExprBeginsWith<'stmt>> for ExprPattern<'stmt> {
    fn from(value: ExprBeginsWith<'stmt>) -> Self {
        ExprPattern::BeginsWith(value)
    }
}
