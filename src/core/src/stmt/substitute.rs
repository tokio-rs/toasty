use super::*;

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr;
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
