mod expr_target;
pub(crate) use expr_target::ExprTarget;

// TODO: don't have these be public.
pub(crate) mod flatten_bool_ops;
pub(crate) mod lift_pk_select;

mod expr_binary_op;
mod expr_cast;
mod expr_concat_str;
mod expr_in_list;
mod expr_is_null;
mod expr_list;
mod expr_map;
mod expr_record;

mod value;

// Simplifications
// TODO: unify names
mod lift_in_subquery;
mod rewrite_root_path_expr;

use toasty_core::{
    schema::*,
    stmt::{self, VisitMut},
};

use std::mem;
use stmt::Expr;

pub(crate) struct Simplify<'a> {
    /// Schema the statement is referencing
    schema: &'a Schema,

    /// The context in which expressions are evaluated. This is a model or
    /// table.
    target: ExprTarget<'a>,
}

// TODO: get rid of this
pub(crate) fn simplify_expr<'a>(
    schema: &'a Schema,
    target: impl Into<ExprTarget<'a>>,
    expr: &mut stmt::Expr,
) {
    Simplify {
        schema,
        target: target.into(),
    }
    .visit_expr_mut(expr);
}

impl<'a> VisitMut for Simplify<'_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        // First, simplify the expression.
        stmt::visit_mut::visit_expr_mut(self, i);

        // If an in-subquery expression, then try lifting it.
        let maybe_expr = match i {
            Expr::BinaryOp(expr_binary_op) => self.simplify_expr_binary_op(
                expr_binary_op.op,
                &mut *expr_binary_op.lhs,
                &mut *expr_binary_op.rhs,
            ),
            Expr::Cast(expr) => self.simplify_expr_cast(expr),
            Expr::ConcatStr(expr) => self.simplify_expr_concat_str(expr),
            Expr::InList(expr) => self.simplify_expr_in_list(expr),
            Expr::InSubquery(expr_in_subquery) => {
                self.lift_in_subquery(&expr_in_subquery.expr, &expr_in_subquery.query)
            }
            Expr::List(expr) => self.simplify_expr_list(expr),
            Expr::Map(_) => self.simplify_expr_map(i),
            Expr::Record(expr) => self.simplify_expr_record(expr),
            Expr::IsNull(expr) => self.simplify_expr_is_null(expr),
            _ => None,
        };

        if let Some(expr) = maybe_expr {
            *i = expr;
        }
    }

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

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        let target = mem::replace(
            &mut self.target,
            ExprTarget::from_source(self.schema, &stmt.from),
        );
        stmt::visit_mut::visit_stmt_delete_mut(self, stmt);
        self.target = target;
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        let target = mem::replace(
            &mut self.target,
            ExprTarget::from_insert_target(self.schema, &stmt.target),
        );
        stmt::visit_mut::visit_stmt_insert_mut(self, stmt);
        self.target = target;
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        let target = mem::replace(
            &mut self.target,
            ExprTarget::from_source(self.schema, &stmt.source),
        );
        stmt::visit_mut::visit_stmt_select_mut(self, stmt);
        self.target = target;
    }

    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        let target = mem::replace(
            &mut self.target,
            ExprTarget::from_update_target(self.schema, &stmt.target),
        );
        stmt::visit_mut::visit_stmt_update_mut(self, stmt);
        self.target = target;
    }

    fn visit_values_mut(&mut self, values: &mut stmt::Values) {
        stmt::visit_mut::visit_values_mut(self, values);

        let width = match &self.target {
            ExprTarget::Const => todo!(),
            ExprTarget::Model(model) => model.fields.len(),
            ExprTarget::Table => todo!(),
            ExprTarget::TableWithColumns(columns) => columns.len(),
        };

        for row in &mut values.rows {
            let actual = match row {
                stmt::Expr::Record(row) => {
                    while row.len() < width {
                        row.push(stmt::Expr::default());
                    }

                    row.len()
                }
                stmt::Expr::Value(stmt::Value::Record(row)) => {
                    while row.len() < width {
                        row.fields.push(stmt::Value::default());
                    }

                    row.len()
                }
                _ => todo!("row={row:#?}"),
            };

            assert_eq!(actual, width, "target={:#?}", self.target);
        }
    }
}

impl<'a> Simplify<'a> {
    pub(crate) fn new(schema: &'a Schema) -> Simplify<'a> {
        Simplify {
            schema,
            target: ExprTarget::Const,
        }
    }

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
