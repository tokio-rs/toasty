use toasty_core::{schema::app::FieldId, stmt};

use crate::engine::simplify::Simplify;

/// Result of extracting a key expression from a subquery.
pub(crate) struct ExtractedKey {
    /// The extracted key expression (e.g., the constant `123`).
    pub expr: stmt::Expr,

    /// Whether additional filter conditions beyond the key equality were
    /// present in the original query. When `true`, the caller must ensure
    /// the additional conditions are preserved as a guard (e.g., by adding
    /// the original query as an `IN` subquery filter on the parent update).
    pub has_extra_conditions: bool,
}

impl Simplify<'_> {
    /// Extracts the constant value from a simple subquery that filters on a key field.
    ///
    /// This helper method analyzes subqueries like `SELECT id FROM users WHERE id = 123`
    /// and extracts the constant value `123` if the pattern matches. The caller uses this
    /// extracted value to eliminate the subquery entirely. Primarily used during belongs-to
    /// relationship planning to extract foreign key values.
    ///
    /// When the filter is a conjunction (e.g., `WHERE id = 123 AND name = "foo"`), the key
    /// value is extracted but `has_extra_conditions` is set to `true` so the caller can
    /// preserve the additional conditions.
    ///
    /// Returns `None` if the subquery pattern doesn't match (e.g., complex filters,
    /// composite keys, non-equality operators).
    pub(crate) fn extract_key_expr(
        &mut self,
        key: &[FieldId],
        stmt: &stmt::Query,
    ) -> Option<ExtractedKey> {
        let cx = self.cx.scope(stmt);

        let stmt::ExprSet::Select(select) = &stmt.body else {
            return None;
        };

        match select.filter.as_expr() {
            stmt::Expr::BinaryOp(expr_binary_op) => self
                .try_extract_key_from_binary_op(&cx, key, expr_binary_op)
                .map(|expr| ExtractedKey {
                    expr,
                    has_extra_conditions: false,
                }),
            stmt::Expr::And(expr_and) => {
                // Search each operand for the key equality condition.
                for operand in &expr_and.operands {
                    if let stmt::Expr::BinaryOp(expr_binary_op) = operand {
                        if let Some(expr) =
                            self.try_extract_key_from_binary_op(&cx, key, expr_binary_op)
                        {
                            return Some(ExtractedKey {
                                expr,
                                has_extra_conditions: true,
                            });
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn try_extract_key_from_binary_op(
        &self,
        cx: &stmt::ExprContext,
        key: &[FieldId],
        expr_binary_op: &stmt::ExprBinaryOp,
    ) -> Option<stmt::Expr> {
        if !expr_binary_op.op.is_eq() {
            return None;
        }

        let [key_field] = key else {
            return None;
        };

        match (&*expr_binary_op.lhs, &*expr_binary_op.rhs) {
            (
                stmt::Expr::Reference(inner @ stmt::ExprReference::Field { nesting: 0, .. }),
                stmt::Expr::Reference(outer @ stmt::ExprReference::Field { nesting, .. }),
            )
            | (
                stmt::Expr::Reference(outer @ stmt::ExprReference::Field { nesting, .. }),
                stmt::Expr::Reference(inner @ stmt::ExprReference::Field { nesting: 0, .. }),
            ) if *nesting > 0 => self.extract_key_expr_nested_ref(cx, *key_field, inner, outer),
            (stmt::Expr::Reference(_), stmt::Expr::Reference(_)) => None,
            (stmt::Expr::Reference(expr_ref), other) | (other, stmt::Expr::Reference(expr_ref)) => {
                let field_ref = cx.resolve_expr_reference(expr_ref).expect_field();

                if *key_field == field_ref.id {
                    if let stmt::Expr::Value(value) = other {
                        Some(value.clone().into())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn extract_key_expr_nested_ref(
        &self,
        cx: &stmt::ExprContext,
        key_field_id: FieldId,
        inner: &stmt::ExprReference,
        outer: &stmt::ExprReference,
    ) -> Option<stmt::Expr> {
        let field_ref = cx.resolve_expr_reference(inner).expect_field();

        if key_field_id == field_ref.id {
            let mut ret = *outer;
            let stmt::ExprReference::Field { nesting, .. } = &mut ret else {
                panic!()
            };
            // This should have been ensured already by the caller
            debug_assert!(*nesting > 0);

            // The returned expression is rescoped to the parent.
            *nesting -= 1;

            Some(ret.into())
        } else {
            None
        }
    }
}
