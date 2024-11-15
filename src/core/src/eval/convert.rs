use super::*;

pub(crate) struct Const;

pub trait Convert<'stmt> {
    fn convert_expr_field(&mut self, field: stmt::ExprField) -> Option<Expr<'stmt>> {
        None
    }
}

impl<'stmt> Convert<'stmt> for Const {}
