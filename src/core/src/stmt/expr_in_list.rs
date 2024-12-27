use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprInList {
    pub expr: Box<Expr>,
    pub list: Box<Expr>,
}

impl Expr {
    pub fn in_list(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprInList {
            expr: Box::new(lhs.into()),
            list: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprInList> for Expr {
    fn from(value: ExprInList) -> Self {
        Expr::InList(value)
    }
}
