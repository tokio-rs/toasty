mod expr_target;
pub(crate) use expr_target::ExprTarget;

// TODO: don't have these be public.
pub(crate) mod flatten_bool_ops;
pub(crate) mod lift_pk_select;

mod expr_binary_op;
mod expr_cast;
mod expr_in_list;
mod expr_record;

mod expr;
use expr::SimplifyExpr;

mod value;

// Simplifications
// TODO: unify names
mod lift_in_subquery;
mod rewrite_root_path_expr;

use super::*;

struct SimplifyStmt<'a> {
    schema: &'a Schema,
}

pub(crate) fn simplify_expr<'a>(
    schema: &'a Schema,
    target: impl Into<ExprTarget<'a>>,
    expr: &mut stmt::Expr,
) {
    SimplifyExpr::new(schema, target).visit_expr_mut(expr);
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

impl<'a> VisitMut for SimplifyStmt<'_> {
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
        SimplifyExpr::new(self.schema, self.schema.model(i.from.as_model_id()))
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

        SimplifyExpr::new(self.schema, self.schema.model(model)).visit_stmt_insert_mut(i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut stmt::Update) {
        SimplifyExpr::new(self.schema, self.schema.model(i.target.as_model_id()))
            .visit_stmt_update_mut(i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut stmt::Select) {
        SimplifyExpr::new(self.schema, self.schema.model(i.source.as_model_id()))
            .visit_mut(&mut i.filter);
    }
}
