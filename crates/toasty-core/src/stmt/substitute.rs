use crate::stmt::visit_mut;

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
