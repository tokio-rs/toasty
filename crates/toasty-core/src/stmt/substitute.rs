use crate::stmt::{visit_mut, ExprSet, Input, Projection, Query, TableDerived, TableRef, Values};

use super::{Expr, Value};

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
        let maybe_substitute = match expr {
            Expr::Project(expr_project) => match &*expr_project.base {
                Expr::Arg(expr_arg) => self.input.resolve_arg(expr_arg, &expr_project.projection),
                Expr::Reference(expr_ref) => {
                    self.input.resolve_ref(expr_ref, &expr_project.projection)
                }
                _ => None,
            },
            Expr::Reference(expr_reference) => self
                .input
                .resolve_ref(expr_reference, &Projection::identity()),
            Expr::Arg(expr_arg) => self.input.resolve_arg(expr_arg, &Projection::identity()),
            _ => None,
        };

        if let Some(substitute) = maybe_substitute {
            *expr = substitute;
        } else {
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
    }

    fn visit_table_ref_mut(&mut self, i: &mut TableRef) {
        if let TableRef::Arg(expr_arg) = i {
            let rows = match self
                .input
                .resolve_arg(expr_arg, &Projection::identity())
                .unwrap()
            {
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
