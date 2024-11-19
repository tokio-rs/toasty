use super::*;

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value;
}

#[derive(Debug)]
pub struct Args<T>(T);

pub fn args<T>(input: T) -> Args<T> {
    Args(input)
}

pub fn const_input() -> impl Input {
    struct Unused;

    impl Input for Unused {
        fn resolve_arg(&mut self, _expr_arg: &ExprArg, _projection: &Projection) -> Value {
            panic!("no input provided")
        }
    }

    Unused
}

impl Input for Args<&[Value]> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value {
        projection.resolve_value(&self.0[expr_arg.position]).clone()
    }
}

impl<'stmt, const N: usize> Input for [&stmt::Expr; N] {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value {
        match projection.resolve_expr(&self[expr_arg.position]) {
            stmt::Expr::Value(value) => value.clone(),
            _ => todo!(),
        }
    }
}
