use super::*;

use stmt::Expr;

impl SimplifyExpr<'_> {
    /// Recursively walk a binary expression in parallel
    pub(super) fn simplify_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
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
            (stmt::Expr::Key(expr_key), other) | (other, stmt::Expr::Key(expr_key)) => {
                assert!(op.is_eq());
                assert!(self.target.is_model());

                // At this point, we must be in a model context, otherwise key
                // expressions don't make sense.
                let ExprTarget::Model(model) = self.target else {
                    todo!()
                };
                Some(self.rewrite_root_path_expr(model, other.take()))
            }
            (stmt::Expr::Field(expr_field), other) | (other, stmt::Expr::Field(expr_field)) => {
                let field = self.schema.field(expr_field.field);

                match &field.ty {
                    FieldTy::Primitive(_) => None,
                    // TODO: Do anything here?
                    FieldTy::HasMany(_) | FieldTy::HasOne(_) => None,
                    FieldTy::BelongsTo(rel) => match op {
                        stmt::BinaryOp::Ne => {
                            let [fk_field, ..] = &rel.foreign_key.fields[..] else {
                                todo!()
                            };

                            assert!(other.is_value_null());

                            expr_field.field = fk_field.source;

                            None
                        }
                        stmt::BinaryOp::Eq => {
                            let [fk_field] = &rel.foreign_key.fields[..] else {
                                todo!()
                            };

                            expr_field.field = fk_field.source;

                            *other = match other.take() {
                                stmt::Expr::Record(_) => todo!(),
                                other => other,
                            };

                            None
                        }
                        _ => todo!("op = {:#?}; lhs={:#?}; rhs={:#?}", op, lhs, rhs),
                    },
                }
            }
            _ => {
                // For now, just make sure there are no relations in the expression
                stmt::visit::for_each_expr(lhs, |expr| {
                    if let stmt::Expr::Project(expr_project) = expr {
                        /*
                        let field = expr_project
                            .projection
                            .resolve_field(self.schema, self.model);
                        assert!(field.ty.is_primitive());
                        */
                        todo!()
                    }
                });

                stmt::visit::for_each_expr(rhs, |expr| {
                    if let stmt::Expr::Project(expr_project) = expr {
                        /*
                        let field = expr_project
                            .projection
                            .resolve_field(self.schema, self.model);
                        assert!(field.ty.is_primitive());
                        */
                        todo!()
                    }
                });

                None
            }
        }
    }
}
