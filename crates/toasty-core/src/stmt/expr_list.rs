use super::*;

#[derive(Debug, Clone)]
pub struct ExprList {
    pub items: Vec<Expr>,
}

impl Expr {
    pub fn list<T>(items: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Self>,
    {
        ExprList {
            items: items.into_iter().map(Into::into).collect(),
        }
        .into()
    }

    pub fn list_from_vec(items: Vec<Self>) -> Self {
        ExprList { items }.into()
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_) | Self::Value(Value::List(_)))
    }

    pub fn is_list_empty(&self) -> bool {
        match self {
            Self::List(list) => list.items.is_empty(),
            Self::Value(Value::List(list)) => list.is_empty(),
            _ => false,
        }
    }
}

impl From<ExprList> for Expr {
    fn from(value: ExprList) -> Self {
        Self::List(value)
    }
}

impl From<Vec<Self>> for Expr {
    fn from(value: Vec<Self>) -> Self {
        Self::list_from_vec(value)
    }
}
