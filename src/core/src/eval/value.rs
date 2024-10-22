use super::*;

impl<'stmt> Expr<'stmt> {
    pub fn null() -> Expr<'stmt> {
        Expr::Value(Value::Null)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Expr::Value(Value::Null))
    }
}
