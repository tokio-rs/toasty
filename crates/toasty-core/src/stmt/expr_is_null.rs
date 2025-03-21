use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprIsNull {
    /// IS NOT NULL
    pub negate: bool,

    /// Expression to check for null
    pub expr: Box<Expr>,
}

impl Expr {
    pub fn is_null(expr: impl Into<Expr>) -> Expr {
        ExprIsNull {
            negate: false,
            expr: Box::new(expr.into()),
        }
        .into()
    }

    pub fn is_not_null(expr: impl Into<Expr>) -> Expr {
        ExprIsNull {
            negate: true,
            expr: Box::new(expr.into()),
        }
        .into()
    }
}

impl From<ExprIsNull> for Expr {
    fn from(value: ExprIsNull) -> Self {
        Expr::IsNull(value)
    }
}
