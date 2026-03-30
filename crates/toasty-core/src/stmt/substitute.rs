//! Expression substitution via the [`VisitMut`](super::VisitMut) pattern.
//!
//! `Substitute` is a [`VisitMut`](super::VisitMut) implementation that
//! replaces `Arg` and `Reference` nodes in an expression tree by resolving
//! them through an [`Input`]. It respects scoping: `Let` bodies and `Map`
//! functions are not recursed into (their local args must remain intact).

use crate::stmt::{
    ExprSet, Input, Projection, Query, TableDerived, TableRef, Values, visit, visit_mut,
};

use super::{Expr, Value};

/// A [`VisitMut`](super::VisitMut) that substitutes `Arg` and `Reference`
/// nodes using an [`Input`].
pub(crate) struct Substitute<I> {
    input: I,
}

impl<I> Substitute<I> {
    /// Creates a new `Substitute` with the given input resolver.
    pub(crate) fn new(input: I) -> Substitute<I> {
        Substitute { input }
    }
}

/// Assert that `expr` only contains `Arg` nodes with `nesting == 0`.
fn assert_only_local_args(expr: &Expr, msg: &str) {
    debug_assert!(
        {
            let mut ok = true;
            visit::for_each_expr(expr, |e| {
                if let Expr::Arg(a) = e {
                    if a.nesting != 0 {
                        ok = false;
                    }
                }
            });
            ok
        },
        "{}",
        msg
    );
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
                Expr::Let(expr_let) => {
                    // Substitute only recurses into the bindings. The body
                    // references the binding results via Arg(nesting=0), so
                    // we must not substitute those.
                    assert_only_local_args(
                        &expr_let.body,
                        "Let body contains args with nesting > 0",
                    );
                    for binding in &mut expr_let.bindings {
                        self.visit_expr_mut(binding);
                    }
                }
                Expr::Map(expr_map) => {
                    // Substitute only recurses into the base. The map body
                    // references the base elements via Arg(nesting=0), so
                    // we must not substitute those.
                    assert_only_local_args(
                        &expr_map.map,
                        "Map body contains args with nesting > 0",
                    );
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
