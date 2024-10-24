pub(crate) mod flatten_bool_ops;
pub(crate) mod lift_pk_select;

mod lift_in_subquery;
mod rewrite_root_path_expr;

use super::*;

struct SimplifyStmt<'a> {
    schema: &'a Schema,
}

struct SimplifyExpr<'a> {
    model: &'a Model,
    schema: &'a Schema,
}

impl<'stmt> Planner<'_, 'stmt> {
    pub(crate) fn simplify_stmt_delete(&self, stmt: &mut stmt::Delete<'stmt>) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_link(&self, stmt: &mut stmt::Link<'stmt>) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_insert(&self, stmt: &mut stmt::Insert<'stmt>) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_query(&self, stmt: &mut stmt::Query<'stmt>) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_unlink(&self, stmt: &mut stmt::Unlink<'stmt>) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_update(&self, stmt: &mut stmt::Update<'stmt>) {
        self.simplify_stmt(stmt);
    }

    fn simplify_stmt<T: stmt::Node<'stmt>>(&self, stmt: &mut T) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_mut(stmt);
    }
}

impl SimplifyStmt<'_> {
    /// Returns the source model
    fn flatten_nested_unions<'stmt>(
        &self,
        expr_set_op: &mut stmt::ExprSetOp<'stmt>,
        operands: &mut Vec<stmt::ExprSet<'stmt>>,
    ) {
        assert!(expr_set_op.is_union());

        for expr_set in &mut expr_set_op.operands {
            match expr_set {
                stmt::ExprSet::SetOp(nested_set_op) if nested_set_op.is_union() => {
                    self.flatten_nested_unions(nested_set_op, operands)
                }
                // Just drop empty values
                stmt::ExprSet::Values(values) if values.is_empty() => {}
                stmt::ExprSet::Select(select) => {
                    if let Some(stmt::ExprSet::Select(tail)) = operands.last_mut() {
                        if tail.source == select.source {
                            assert_eq!(select.returning, tail.returning);

                            tail.or(select.filter.take());
                            continue;
                        }
                    }

                    operands.push(std::mem::take(expr_set));
                }
                stmt::ExprSet::Values(values) => {
                    if let Some(stmt::ExprSet::Values(tail)) = operands.last_mut() {
                        tail.rows.extend(values.rows.drain(..));
                        continue;
                    }

                    operands.push(std::mem::take(expr_set));
                }
                _ => todo!("expr={:#?}", expr_set),
            }
        }
    }
}

impl<'a, 'stmt> VisitMut<'stmt> for SimplifyStmt<'_> {
    fn visit_expr_set_mut(&mut self, i: &mut stmt::ExprSet<'stmt>) {
        match i {
            stmt::ExprSet::SetOp(expr_set_op) if expr_set_op.operands.len() == 0 => {
                todo!("is there anything we do here?");
            }
            stmt::ExprSet::SetOp(expr_set_op) if expr_set_op.operands.len() == 1 => {
                let operand = expr_set_op.operands.drain(..).next().unwrap();
                *i = operand;
            }
            stmt::ExprSet::SetOp(expr_set_op) if expr_set_op.is_union() => {
                // First, simplify each sub-query in the union, then rewrite the
                // query as a single disjuntive query.
                let mut operands = vec![];

                self.flatten_nested_unions(expr_set_op, &mut operands);

                expr_set_op.operands = operands;
            }
            _ => {}
        }

        stmt::visit_mut::visit_expr_set_mut(self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut stmt::Insert<'stmt>) {
        self.visit_stmt_query_mut(&mut i.scope);

        let mut simplify_expr = SimplifyExpr {
            model: self
                .schema
                .model(i.scope.body.as_select().source.as_model_id()),
            schema: self.schema,
        };

        simplify_expr.visit_expr_mut(&mut i.values);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut stmt::Update<'stmt>) {
        self.visit_stmt_query_mut(&mut i.selection);

        let mut simplify_expr = SimplifyExpr {
            model: self
                .schema
                .model(i.selection.body.as_select().source.as_model_id()),
            schema: self.schema,
        };

        for expr in i.expr.iter_mut() {
            simplify_expr.visit_expr_mut(expr);
        }

        if let Some(expr) = &mut i.condition {
            simplify_expr.visit_expr_mut(expr);
        }
    }

    fn visit_stmt_select_mut(&mut self, i: &mut stmt::Select<'stmt>) {
        SimplifyExpr {
            model: self.schema.model(i.source.as_model_id()),
            schema: self.schema,
        }
        .visit_mut(&mut i.filter);
    }

    fn visit_expr_mut(&mut self, _i: &mut stmt::Expr<'stmt>) {
        panic!("should not be reached")
    }
}

impl SimplifyExpr<'_> {
    /// Recursively walk a binary expression in parallel
    fn simplify_binary_op<'stmt>(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr<'stmt>,
        rhs: &mut stmt::Expr<'stmt>,
    ) -> Option<stmt::Expr<'stmt>> {
        match (&mut *lhs, &mut *rhs) {
            (stmt::Expr::Project(expr_project), other)
            | (other, stmt::Expr::Project(expr_project)) => {
                if expr_project.is_identity() {
                    assert!(op.is_eq());

                    Some(self.rewrite_root_path_expr(other.clone()))
                } else {
                    assert!(expr_project.base.is_expr_self());

                    let field = expr_project
                        .projection
                        .resolve_field(self.schema, &self.model);

                    match &field.ty {
                        FieldTy::Primitive(_) => None,
                        // TODO: Do anything here?
                        FieldTy::HasMany(_) | FieldTy::HasOne(_) => None,
                        FieldTy::BelongsTo(rel) => match op {
                            stmt::BinaryOp::Ne => {
                                let [fk_field, ..] = &rel.foreign_key.fields[..] else {
                                    todo!()
                                };

                                assert!(other.is_null());

                                expr_project.projection =
                                    stmt::Projection::from_index(fk_field.source.index);

                                None
                            }
                            stmt::BinaryOp::Eq => {
                                let [fk_field] = &rel.foreign_key.fields[..] else {
                                    todo!()
                                };

                                expr_project.projection =
                                    stmt::Projection::from_index(fk_field.source.index);

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
            }
            _ => {
                // For now, just make sure there are no relations in the expression
                stmt::visit::for_each_expr(lhs, |expr| {
                    if let stmt::Expr::Project(expr_project) = expr {
                        let field = expr_project
                            .projection
                            .resolve_field(&self.schema, &self.model);
                        assert!(field.ty.is_primitive());
                    }
                });

                stmt::visit::for_each_expr(rhs, |expr| {
                    if let stmt::Expr::Project(expr_project) = expr {
                        let field = expr_project
                            .projection
                            .resolve_field(&self.schema, &self.model);
                        assert!(field.ty.is_primitive());
                    }
                });

                None
            }
        }
    }
}

impl<'stmt> VisitMut<'stmt> for SimplifyExpr<'_> {
    fn visit_stmt_mut(&mut self, _i: &mut stmt::Statement<'stmt>) {
        panic!("should not be reached");
    }

    fn visit_expr_project_mut(&mut self, i: &mut stmt::ExprProject<'stmt>) {
        assert!(i.projection.len() <= 1);
    }

    fn visit_expr_mut(&mut self, i: &mut stmt::Expr<'stmt>) {
        // First, simplify the expression.
        stmt::visit_mut::visit_expr_mut(self, i);

        // If an in-subquery expression, then try lifting it.
        match i {
            stmt::Expr::InSubquery(expr_in_subquery) => {
                if let Some(mut lifted) = self.lift_in_subquery(
                    self.model,
                    &expr_in_subquery.expr,
                    &expr_in_subquery.query,
                ) {
                    stmt::visit_mut::visit_expr_mut(self, &mut lifted);
                    *i = lifted;
                }
            }
            stmt::Expr::BinaryOp(expr_binary_op) => {
                let ret = self.simplify_binary_op(
                    expr_binary_op.op,
                    &mut *expr_binary_op.lhs,
                    &mut *expr_binary_op.rhs,
                );

                if let Some(ret) = ret {
                    *i = ret;
                }

                /*
                if let stmt::Expr::Project(lhs) = &*expr_binary_op.lhs {
                    if lhs.projection.is_identity() {
                        assert!(expr_binary_op.op.is_eq());
                        let rhs = std::mem::take(&mut *expr_binary_op.rhs);
                        *i = self.rewrite_root_path_expr(rhs);
                        return;
                    }
                }

                if let stmt::Expr::Project(rhs) = &*expr_binary_op.lhs {
                    if rhs.projection.is_identity() {
                        todo!()
                    }
                }
                */
            }
            _ => {}
        }
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt<'stmt>) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_stmt_mut(&mut *i.stmt);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query<'stmt>) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_stmt_query_mut(i);
    }
}
