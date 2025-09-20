use toasty_core::stmt;

pub trait Convert {
    fn convert_expr_reference(&mut self, _expr_ref: &stmt::ExprReference) -> Option<stmt::Expr> {
        None
    }

    fn convert_expr_column(&mut self, _stmt: &stmt::ExprColumn) -> Option<stmt::Expr> {
        None
    }
}
