use super::*;

pub trait Input<'stmt> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value<'stmt>;
}

#[derive(Debug)]
pub struct Args<T>(T);

pub fn args<T>(input: T) -> Args<T> {
    Args(input)
}

pub fn const_input<'stmt>() -> impl Input<'stmt> {
    struct Unused;

    impl<'stmt> Input<'stmt> for Unused {
        fn resolve_arg(&mut self, _expr_arg: &ExprArg, _projection: &Projection) -> Value<'stmt> {
            panic!("no input provided")
        }
    }

    Unused
}

impl<'stmt> Input<'stmt> for Args<&[Value<'stmt>]> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value<'stmt> {
        projection.resolve_value(&self.0[expr_arg.position]).clone()
    }
}

impl<'stmt, const N: usize> Input<'stmt> for [&stmt::Expr<'stmt>; N] {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value<'stmt> {
        match projection.resolve_expr(&self[expr_arg.position]) {
            stmt::Expr::Value(value) => value.clone(),
            _ => todo!(),
        }
    }
}
