use crate::stmt::{visit_mut, ExprSet, Query, TableDerived, TableRef, Values};

use super::{Expr, ExprArg, Value};

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr;
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

        // Substitute after recurring.
        if let Expr::Arg(expr_arg) = expr {
            *expr = self.input.resolve_arg(expr_arg);
        }
    }

    fn visit_table_ref_mut(&mut self, i: &mut TableRef) {
        if let TableRef::Arg(expr_arg) = i {
            let rows = match self.input.resolve_arg(expr_arg) {
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
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr {
        self[expr_arg.position].clone().into()
    }
}

impl<const N: usize> Input for &[Value; N] {
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr {
        self[expr_arg.position].clone().into()
    }
}
