use toasty_core::{schema::app::FieldId, stmt};

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    /// Extracts the constant value from a simple subquery that filters on a key field.
    ///
    /// This helper method analyzes subqueries like `SELECT id FROM users WHERE id = 123`
    /// and extracts the constant value `123` if the pattern matches. The caller uses this
    /// extracted value to eliminate the subquery entirely. Primarily used during belongs-to
    /// relationship planning to extract foreign key values.
    ///
    /// Example usage by caller:
    /// ```sql
    /// -- Subquery analyzed by this method
    /// (SELECT id FROM users WHERE id = 123)
    ///
    /// -- If this method returns Some(123), caller replaces subquery with:
    /// 123
    /// ```
    ///
    /// Returns `None` if the subquery pattern doesn't match (e.g., complex filters,
    /// composite keys, non-equality operators).
    pub(crate) fn extract_key_value(
        &mut self,
        key: &[FieldId],
        stmt: &stmt::Query,
    ) -> Option<stmt::Expr> {
        let stmt::ExprSet::Select(select) = &stmt.body else {
            return None;
        };

        let model = self.schema.app.model(select.source.as_model_id());

        match &select.filter {
            stmt::Expr::BinaryOp(expr_binary_op) => {
                if !expr_binary_op.op.is_eq() {
                    return None;
                }

                let [key_field] = key else {
                    return None;
                };

                let expr_reference = match &*expr_binary_op.lhs {
                    stmt::Expr::Reference(expr_ref) => expr_ref,
                    _ => return None,
                };
                let Some(lhs_field) = self.resolve_expr_reference(expr_reference) else {
                    todo!("handle None");
                };

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
}
