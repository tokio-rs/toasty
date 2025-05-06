use super::*;

#[derive(Debug, Clone)]
pub struct ExprBeginsWith {
    pub expr: Box<Expr>,
    pub pattern: Box<Expr>,
}

impl Expr {
    pub fn begins_with(expr: impl Into<Self>, pattern: impl Into<Self>) -> Self {
        ExprBeginsWith {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl From<ExprBeginsWith> for Expr {
    fn from(value: ExprBeginsWith) -> Self {
        Self::Pattern(value.into())
    }
}

impl From<ExprBeginsWith> for ExprPattern {
    fn from(value: ExprBeginsWith) -> Self {
        Self::BeginsWith(value)
    }
}
