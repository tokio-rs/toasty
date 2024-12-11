use super::*;

pub trait Convert {
    fn convert_expr_field(&mut self, stmt: stmt::ExprField) -> Expr {
        todo!("convert_expr_field; {stmt:#?}");
    }

    fn convert_expr_column(&mut self, stmt: stmt::ExprColumn) -> Expr {
        todo!("convert_expr_column; {stmt:#?}");
    }
}

pub(super) struct ConstExpr;

impl Convert for ConstExpr {}
