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
    pub(crate) fn extract_key_expr(
        &mut self,
        key: &[FieldId],
        stmt: &stmt::Query,
    ) -> Option<stmt::Expr> {
        let cx = self.cx.scope(stmt);

        let stmt::ExprSet::Select(select) = &stmt.body else {
            return None;
        };

        match select.filter.as_expr() {
            stmt::Expr::BinaryOp(expr_binary_op) => {
                if !expr_binary_op.op.is_eq() {
                    return None;
                }

                let [key_field] = key else {
                    return None;
                };

                match (&*expr_binary_op.lhs, &*expr_binary_op.rhs) {
                    (
                        stmt::Expr::Reference(
                            inner @ stmt::ExprReference::Field { nesting: 0, .. },
                        ),
                        stmt::Expr::Reference(outer @ stmt::ExprReference::Field { nesting, .. }),
                    )
                    | (
                        stmt::Expr::Reference(outer @ stmt::ExprReference::Field { nesting, .. }),
                        stmt::Expr::Reference(
                            inner @ stmt::ExprReference::Field { nesting: 0, .. },
                        ),
                    ) if *nesting > 0 => {
                        self.extract_key_expr_nested_ref(&cx, *key_field, inner, outer)
                    }
                    (stmt::Expr::Reference(_), stmt::Expr::Reference(_)) => {
                        todo!("stmt={stmt:#?}");
                    }
                    (stmt::Expr::Reference(expr_ref), other)
                    | (other, stmt::Expr::Reference(expr_ref)) => {
                        let field_ref = cx.resolve_expr_reference(expr_ref).expect_field();

                        if *key_field == field_ref.id {
                            if let stmt::Expr::Value(value) = other {
                                Some(value.clone().into())
                            } else {
                                todo!()
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            stmt::Expr::And(_) => {
                todo!("either support PKs or check each op for the key");
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
