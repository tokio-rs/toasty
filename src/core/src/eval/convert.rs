use super::*;

pub trait Convert {
    fn convert_expr_field(&mut self, field: stmt::ExprField) -> Expr;
}

impl Convert for () {
    fn convert_expr_field(&mut self, field: stmt::ExprField) -> Expr {
        todo!()
    }
}
