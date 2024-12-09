use super::*;

pub trait Convert {
    fn convert_expr_field(&mut self, field: stmt::ExprField) -> Expr;
}

pub(super) struct ConstExpr;

impl Convert for ConstExpr {
    fn convert_expr_field(&mut self, field: stmt::ExprField) -> Expr {
        todo!()
    }
}
