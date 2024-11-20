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

impl Planner<'_> {
    pub(crate) fn simplify_stmt_delete(&self, stmt: &mut stmt::Delete) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_link(&self, stmt: &mut stmt::Link) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_insert(&self, stmt: &mut stmt::Insert) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_query(&self, stmt: &mut stmt::Query) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_unlink(&self, stmt: &mut stmt::Unlink) {
        self.simplify_stmt(stmt);
    }

    pub(crate) fn simplify_stmt_update(&self, stmt: &mut stmt::Update) {
        self.simplify_stmt(stmt);
    }

    fn simplify_stmt<T: stmt::Node>(&self, stmt: &mut T) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_mut(stmt);
    }
}

impl SimplifyStmt<'_> {
    /// Returns the source model
    fn flatten_nested_unions(
        &self,
        expr_set_op: &mut stmt::ExprSetOp,
        operands: &mut Vec<stmt::ExprSet>,
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

impl<'a, 'stmt> VisitMut for SimplifyStmt<'_> {
    fn visit_expr_set_mut(&mut self, i: &mut stmt::ExprSet) {
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

    fn visit_stmt_delete_mut(&mut self, i: &mut stmt::Delete) {
        SimplifyExpr {
            model: self.schema.model(i.from.as_model_id()),
            schema: self.schema,
        }
        .visit_mut(&mut i.filter);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut stmt::Insert) {
        let model = match &mut i.target {
            stmt::InsertTarget::Scope(query) => {
                self.visit_stmt_query_mut(query);
                query.body.as_select().source.as_model_id()
            }
            stmt::InsertTarget::Model(model) => *model,
            _ => todo!(),
        };

        // Make sure rows are the right size
        if let stmt::ExprSet::Values(values) = &mut *i.source.body {
            let model = self.schema.model(model);

            for row in &mut values.rows {
                let stmt::Expr::Record(row) = row else {
                    todo!()
                };

                while row.len() < model.fields.len() {
                    row.push(stmt::Expr::default());
                }
            }
        }

        let mut simplify_expr = SimplifyExpr {
            model: self.schema.model(model),
            schema: self.schema,
        };

        simplify_expr.visit_stmt_insert_mut(i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut stmt::Update) {
        let mut simplify_expr = SimplifyExpr {
            model: self.schema.model(i.target.as_model_id()),
            schema: self.schema,
        };

        simplify_expr.visit_stmt_update_mut(i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut stmt::Select) {
        SimplifyExpr {
            model: self.schema.model(i.source.as_model_id()),
            schema: self.schema,
        }
        .visit_mut(&mut i.filter);
    }
}

impl SimplifyExpr<'_> {
    /// Recursively walk a binary expression in parallel
    fn simplify_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *lhs, &mut *rhs) {
            (stmt::Expr::Key(expr_key), other) | (other, stmt::Expr::Key(expr_key)) => {
                assert!(op.is_eq());
                Some(self.rewrite_root_path_expr(other.take()))
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

                            assert!(other.is_null());

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
                        let field = expr_project
                            .projection
                            .resolve_field(self.schema, self.model);
                        assert!(field.ty.is_primitive());
                    }
                });

                stmt::visit::for_each_expr(rhs, |expr| {
                    if let stmt::Expr::Project(expr_project) = expr {
                        let field = expr_project
                            .projection
                            .resolve_field(self.schema, self.model);
                        assert!(field.ty.is_primitive());
                    }
                });

                None
            }
        }
    }
}

impl VisitMut for SimplifyExpr<'_> {
    fn visit_stmt_mut(&mut self, _i: &mut stmt::Statement) {
        panic!("should not be reached");
    }

    fn visit_expr_project_mut(&mut self, i: &mut stmt::ExprProject) {
        assert!(i.projection.len() <= 1);
    }

    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
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

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_stmt_mut(&mut *i.stmt);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_stmt_query_mut(i);
    }
}
