use super::*;

pub enum Entry<'a> {
    Expr(&'a Expr),
    Value(&'a Value),
}

impl<'a> Entry<'a> {
    pub fn is_expr(&self) -> bool {
        matches!(self, Entry::Expr(_))
    }

    pub fn is_value(&self) -> bool {
        matches!(self, Entry::Value(_))
    }

    pub fn is_value_null(&self) -> bool {
        matches!(self, Entry::Value(Value::Null))
    }
}

impl<'a> From<&'a Expr> for Entry<'a> {
    fn from(value: &'a Expr) -> Self {
        match value {
            Expr::Value(value) => Entry::Value(value),
            _ => Entry::Expr(value),
        }
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
