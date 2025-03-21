use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprBeginsWith {
    pub expr: Box<Expr>,
    pub pattern: Box<Expr>,
}

impl Expr {
    pub fn begins_with(expr: impl Into<Expr>, pattern: impl Into<Expr>) -> Expr {
        ExprBeginsWith {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl From<ExprBeginsWith> for Expr {
    fn from(value: ExprBeginsWith) -> Self {
        Expr::Pattern(value.into())
    }
}

impl From<ExprBeginsWith> for ExprPattern {
    fn from(value: ExprBeginsWith) -> Self {
        ExprPattern::BeginsWith(value)
    }
}
