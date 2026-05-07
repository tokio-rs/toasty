use toasty_core::{
    schema::app::FieldTy,
    stmt::{self, ExprContext, IntoExprTarget, VisitMut},
};

/// Pre-lowering pass that rewrites the operands of `eq` and `ne` binary
/// operators when they reference an app-level construct that lowering will
/// later remove.
///
/// Two rewrites fire here:
///
/// - `Reference::Model { nesting }` becomes a reference to the model's
///   primary-key field.  This lets `WHERE user = $u` (where `$u` is a model
///   value) compile to `WHERE user.id = $u.id` once the value-record
///   decomposition runs in simplify.
/// - `Reference::Field` to a `BelongsTo` relation becomes a reference to the
///   relation's foreign-key field.  This lets `WHERE post.author = $u`
///   compile to `WHERE post.author_id = $u.id`.
///
/// Both rewrites only match on app-level shapes (`Reference::Model`,
/// `BelongsTo` field references); after lowering, every reference is a
/// column reference and these rewrites are no-ops.
///
/// Scope management mirrors `simplify::Simplify`: each statement-level
/// visit pushes the source's scope onto the expression context before
/// visiting filter and returning clauses, so that `resolve_expr_reference`
/// can resolve field references against the right model.
pub(super) struct RewriteEqOperand<'a> {
    cx: ExprContext<'a>,
}

impl<'a> RewriteEqOperand<'a> {
    pub(super) fn new(cx: ExprContext<'a>) -> Self {
        Self { cx }
    }

    /// Walk a statement and apply the eq-operand rewrite to every binary op.
    pub(super) fn rewrite(&mut self, stmt: &mut stmt::Statement) {
        self.visit_mut(stmt);
    }

    fn scope<'scope>(
        &'scope self,
        target: impl IntoExprTarget<'scope>,
    ) -> RewriteEqOperand<'scope> {
        RewriteEqOperand {
            cx: self.cx.scope(target),
        }
    }

    fn rewrite_operand(&self, operand: &mut stmt::Expr) {
        if let stmt::Expr::Reference(expr_reference) = operand {
            match &*expr_reference {
                stmt::ExprReference::Model { nesting } => {
                    let model = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .as_model_unwrap();

                    let [pk_field] = &model.primary_key.fields[..] else {
                        todo!("handle composite keys");
                    };

                    *operand = stmt::Expr::ref_field(*nesting, pk_field);
                }
                stmt::ExprReference::Field { .. } => {
                    let field = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .as_field_unwrap();

                    match &field.ty {
                        FieldTy::Primitive(_) | FieldTy::Embedded(_) => {}
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
    }
}

impl VisitMut for RewriteEqOperand<'_> {
    fn visit_expr_binary_op_mut(&mut self, i: &mut stmt::ExprBinaryOp) {
        // Recurse into children first so nested binary ops get their
        // operands rewritten before this node's operands are inspected.
        stmt::visit_mut::visit_expr_binary_op_mut(self, i);

        if i.op.is_eq() || i.op.is_ne() {
            self.rewrite_operand(&mut i.lhs);
            self.rewrite_operand(&mut i.rhs);
        }
    }

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        self.visit_source_mut(&mut stmt.from);

        let mut s = self.scope(&stmt.from);

        s.visit_filter_mut(&mut stmt.filter);

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        self.visit_insert_target_mut(&mut stmt.target);

        let mut s = self.scope(&stmt.target);

        s.visit_stmt_query_mut(&mut stmt.source);

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        self.visit_source_mut(&mut stmt.source);

        let mut s = self.scope(&stmt.source);

        s.visit_filter_mut(&mut stmt.filter);
        s.visit_returning_mut(&mut stmt.returning);
    }

    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        self.visit_update_target_mut(&mut stmt.target);

        let mut s = self.scope(&stmt.target);

        s.visit_assignments_mut(&mut stmt.assignments);
        s.visit_filter_mut(&mut stmt.filter);

        if let Some(expr) = &mut stmt.condition.expr {
            s.visit_expr_mut(expr);
        }

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }
}
