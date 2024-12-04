use super::*;

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value;
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

impl<const N: usize> Input for [&stmt::Expr; N] {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value {
        self[expr_arg.position].entry(projection).to_value()
    }
}

impl<const N: usize> Input for &[stmt::Value; N] {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Value {
        projection.resolve_value(&self[expr_arg.position]).clone()
    }
}
