use super::*;

impl Expr {
    pub fn null() -> Expr {
        Expr::Value(Value::Null)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Expr::Value(Value::Null))
    }
}
