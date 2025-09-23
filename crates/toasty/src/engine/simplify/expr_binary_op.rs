use super::Simplify;
use toasty_core::{
    schema::app::FieldTy,
    stmt::{self, Expr},
};

impl Simplify<'_> {
    pub(super) fn simplify_expr_eq_operand(&mut self, operand: &mut stmt::Expr) {
        match operand {
            stmt::Expr::Reference(expr_reference) if expr_reference.is_model() => {
                let model = self
                    .cx
                    .resolve_expr_reference(expr_reference)
                    .expect_model();

                let [pk_field] = &model.primary_key.fields[..] else {
                    todo!("handle composite keys");
                };

                *operand = stmt::Expr::ref_field(expr_reference.nesting(), pk_field);
            }
            stmt::Expr::Reference(expr_reference) if expr_reference.is_field() => {
                let field = self
                    .cx
                    .resolve_expr_reference(expr_reference)
                    .expect_field();

                match &field.ty {
                    FieldTy::Primitive(_) => {}
                    FieldTy::HasMany(_) | FieldTy::HasOne(_) => todo!(),
                    FieldTy::BelongsTo(rel) => {
                        let [fk_field] = &rel.foreign_key.fields[..] else {
                            todo!("handle composite keys");
                        };

                        let stmt::ExprReference::Field { index, .. } = expr_reference else {
                            panic!()
                        };
                        *index = fk_field.source.index;
                    }
                }
            }
            _ => {}
        }
    }

    /// Recursively walk a binary expression in parallel
    pub(super) fn simplify_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        if op.is_eq() || op.is_ne() {
            self.simplify_expr_eq_operand(lhs);
            self.simplify_expr_eq_operand(rhs);
        }

        match (&mut *lhs, &mut *rhs) {
            (Expr::Cast(cast), Expr::Value(val)) if cast.ty.is_id() => {
                *lhs = cast.expr.take();
                self.uncast_value_id(val);
                None
            }
            (Expr::Value(val), Expr::Cast(cast)) if cast.ty.is_id() => {
                *rhs = cast.expr.take();
                self.uncast_value_id(val);
                None
            }
            (stmt::Expr::Key(_), other) | (other, stmt::Expr::Key(_)) => {
                assert!(op.is_eq());

                // At this point, we must be in a model context, otherwise key
                // expressions don't make sense.
                let Some(model) = self.cx.target_as_model() else {
                    todo!();
                };

                Some(self.rewrite_root_path_expr(model, other.take()))
            }
            _ => {
                // For now, just make sure there are no relations in the expression
                stmt::visit::for_each_expr(lhs, |expr| {
                    if let stmt::Expr::Project(_) = expr {
                        todo!()
                    }
                });

                stmt::visit::for_each_expr(rhs, |expr| {
                    if let stmt::Expr::Project(_) = expr {
                        todo!()
                    }
                });

                None
            }
        }
    }
}
