use crate::stmt::{visit_mut, ExprSet, Projection, Query, TableDerived, TableRef, Values};

use super::{Expr, ExprArg, Value};

pub trait Input {
    fn resolve_arg_as_expr(&mut self, expr_arg: &ExprArg) -> Expr {
        self.resolve_arg_projected_as_expr(expr_arg, &Projection::identity())
    }

    fn resolve_arg_projected_as_expr(
        &mut self,
        expr_arg: &ExprArg,
        projection: &Projection,
    ) -> Expr;
}

pub(crate) struct Substitute<I> {
    input: I,
}

impl<I> Substitute<I> {
    pub(crate) fn new(input: I) -> Substitute<I> {
        Substitute { input }
    }
}

impl<I> visit_mut::VisitMut for Substitute<I>
where
    I: Input,
{
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        // Substitute first
        match expr {
            Expr::Project(expr_project) => {
                if let Expr::Arg(expr_arg) = &*expr_project.base {
                    *expr = self
                        .input
                        .resolve_arg_projected_as_expr(expr_arg, &expr_project.projection);
                }
            }
            Expr::Arg(expr_arg) => {
                *expr = self.input.resolve_arg_as_expr(expr_arg);
            }
            _ => {}
        }

        // Recurse into child expressions
        match expr {
            Expr::Map(expr_map) => {
                // Only recurse into the base expression as arguments
                // reference the base.
                self.visit_expr_mut(&mut expr_map.base);
            }
            _ => {
                visit_mut::visit_expr_mut(self, expr);
            }
        }
    }

    fn visit_table_ref_mut(&mut self, i: &mut TableRef) {
        if let TableRef::Arg(expr_arg) = i {
            let rows = match self.input.resolve_arg_as_expr(expr_arg) {
                Expr::List(expr_list) => expr_list.items,
                Expr::Value(Value::List(value_list)) => {
                    // TODO: this conversion is not ideal
                    value_list.into_iter().map(Expr::from).collect()
                }
                substitution => panic!(
                    "unexpected substitution; table_ref={i:#?}; substitution={substitution:#?}"
                ),
            };

            *i = TableRef::Derived(TableDerived {
                subquery: Box::new(Query {
                    with: None,
                    body: ExprSet::Values(Values { rows }),
                    single: false,
                    order_by: None,
                    limit: None,
                    locks: vec![],
                }),
            });
        } else {
            visit_mut::visit_table_ref_mut(self, i);
        }
    }
}

impl Input for &Vec<Value> {
    fn resolve_arg_projected_as_expr(
        &mut self,
        expr_arg: &ExprArg,
        projection: &Projection,
    ) -> Expr {
        self[expr_arg.position].entry(projection).to_expr()
    }
}

impl<T: AsRef<Value>, const N: usize> Input for [T; N] {
    fn resolve_arg_projected_as_expr(
        &mut self,
        expr_arg: &ExprArg,
        projection: &Projection,
    ) -> Expr {
        self[expr_arg.position].as_ref().entry(projection).to_expr()
    }
}

impl<T: AsRef<Value>, const N: usize> Input for &[T; N] {
    fn resolve_arg_projected_as_expr(
        &mut self,
        expr_arg: &ExprArg,
        projection: &Projection,
    ) -> Expr {
        self[expr_arg.position].as_ref().entry(projection).to_expr()
    }
}
