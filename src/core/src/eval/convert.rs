use super::*;

pub(crate) struct Const;

pub trait Convert {
    fn convert_expr_field(&mut self, field: stmt::ExprField) -> Option<Expr> {
        None
    }
}

impl Convert for Const {}
