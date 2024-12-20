use super::*;

pub trait Convert {
    fn convert_expr_field(&mut self, stmt: stmt::ExprField) -> Option<stmt::Expr> {
        None
    }

    fn convert_expr_column(&mut self, stmt: stmt::ExprColumn) -> Option<stmt::Expr> {
        None
    }
}

pub(super) struct ConstExpr;

impl Convert for ConstExpr {}
