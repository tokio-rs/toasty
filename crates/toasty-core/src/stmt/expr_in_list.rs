use super::*;

#[derive(Debug, Clone)]
pub struct ExprInList {
    pub expr: Box<Expr>,
    pub list: Box<Expr>,
}

impl Expr {
    pub fn in_list(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        ExprInList {
            expr: Box::new(lhs.into()),
            list: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprInList> for Expr {
    fn from(value: ExprInList) -> Self {
        Self::InList(value)
    }
}
