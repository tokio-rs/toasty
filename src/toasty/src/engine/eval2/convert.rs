use super::*;

pub trait Convert {
    fn convert_expr_field(&mut self, _stmt: &stmt::ExprField) -> Option<stmt::Expr> {
        None
    }

    fn convert_expr_column(&mut self, _stmt: &stmt::ExprColumn) -> Option<stmt::Expr> {
        None
    }
}
