use super::*;
use app::FieldId;

use toasty_core::schema::*;

// TODO: move this to a better location
pub(crate) fn lift_key_select(
    schema: &Schema,
    key: &[FieldId],
    stmt: &stmt::Query,
) -> Option<stmt::Expr> {
    let stmt::ExprSet::Select(select) = &stmt.body else {
        return None;
    };

    let model = schema.app.model(select.source.as_model_id());

    match &select.filter {
        stmt::Expr::BinaryOp(expr_binary_op) => {
            if !expr_binary_op.op.is_eq() {
                return None;
            }

            let [key_field] = key else {
                return None;
            };

            let lhs_field = schema.app.field(expr_binary_op.lhs.as_field().field);

            if *key_field == lhs_field.id {
                if let stmt::Expr::Value(value) = &*expr_binary_op.rhs {
                    Some(value.clone().into())
                } else {
                    todo!()
                }
            } else {
                None
            }
        }
        stmt::Expr::And(_) => {
            if model.primary_key.fields.len() > 1 {
                todo!("support composite keys");
            }

            None
        }
        _ => None,
    }
}
