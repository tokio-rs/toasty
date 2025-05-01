use super::*;

#[derive(Debug)]
pub enum Entry<'a> {
    Expr(&'a Expr),
    Value(&'a Value),
}

impl Entry<'_> {
    pub fn is_expr(&self) -> bool {
        matches!(self, Entry::Expr(_))
    }

    pub fn to_expr(&self) -> Expr {
        match *self {
            Entry::Expr(expr) => expr.clone(),
            Entry::Value(value) => value.clone().into(),
        }
    }

    pub fn is_value(&self) -> bool {
        matches!(self, Entry::Value(_) | Entry::Expr(Expr::Value(_)))
    }

    pub fn is_value_null(&self) -> bool {
        matches!(
            self,
            Entry::Value(Value::Null) | Entry::Expr(Expr::Value(Value::Null))
        )
    }

    pub fn try_as_value(&self) -> Option<&Value> {
        match *self {
            Entry::Expr(Expr::Value(value)) | Entry::Value(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_value(&self) -> &Value {
        match *self {
            Entry::Expr(Expr::Value(value)) | Entry::Value(value) => value,
            _ => todo!(),
        }
    }

    pub fn to_value(&self) -> Value {
        match *self {
            Entry::Expr(Expr::Value(value)) | Entry::Value(value) => value.clone(),
            _ => todo!("entry={self:#?}"),
        }
    }
}

impl<'a> From<&'a Expr> for Entry<'a> {
    fn from(value: &'a Expr) -> Self {
        Entry::Expr(value)
    }
}

impl<'a> From<&'a Value> for Entry<'a> {
    fn from(value: &'a Value) -> Self {
        Entry::Value(value)
    }
}

impl<'a> From<Entry<'a>> for Expr {
    fn from(value: Entry<'a>) -> Self {
        match value {
            Entry::Expr(expr) => expr.clone(),
            Entry::Value(value) => value.clone().into(),
        }
    }
}
