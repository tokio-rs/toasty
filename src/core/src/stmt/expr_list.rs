use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprList {
    pub items: Vec<Expr>,
}

impl Expr {
    pub fn list<T>(items: impl IntoIterator<Item = T>) -> Expr
    where
        T: Into<Expr>,
    {
        ExprList {
            items: items.into_iter().map(Into::into).collect(),
        }
        .into()
    }

    pub fn list_from_vec(items: Vec<Expr>) -> Expr {
        ExprList { items }.into()
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Expr::List(_) | Expr::Value(Value::List(_)))
    }

    pub fn is_list_empty(&self) -> bool {
        match self {
            Expr::List(list) => list.items.is_empty(),
            Expr::Value(Value::List(list)) => list.is_empty(),
            _ => false,
        }
    }
}

impl From<ExprList> for Expr {
    fn from(value: ExprList) -> Self {
        Expr::List(value)
    }
}

impl From<Vec<Expr>> for Expr {
    fn from(value: Vec<Expr>) -> Self {
        Expr::list_from_vec(value)
    }
}
