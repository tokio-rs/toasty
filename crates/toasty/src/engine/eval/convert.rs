use super::*;
use toasty_core::schema::app::FieldId;

pub trait Convert {
    fn convert_expr_field(&mut self, _field_id: FieldId) -> Option<stmt::Expr> {
        None
    }

    fn convert_expr_column(&mut self, _stmt: &stmt::ExprColumn) -> Option<stmt::Expr> {
        None
    }
}
