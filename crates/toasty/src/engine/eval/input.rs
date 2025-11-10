use toasty_core::stmt::{self, Projection, Value};

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &stmt::ExprArg, projection: &Projection) -> Value;

    fn resolve_expr_reference(
        &mut self,
        expr_reference: &stmt::ExprReference,
        projection: &Projection,
    ) -> Option<Value> {
        let _ = (expr_reference, projection);
        None
    }
}

pub fn const_input() -> impl Input {
    struct Unused;

    impl Input for Unused {
        fn resolve_arg(&mut self, _expr_arg: &stmt::ExprArg, _projection: &Projection) -> Value {
            panic!("no input provided")
        }
    }

    Unused
}

impl<const N: usize> Input for [&stmt::Expr; N] {
    fn resolve_arg(&mut self, expr_arg: &stmt::ExprArg, projection: &Projection) -> Value {
        self[expr_arg.position].entry(projection).to_value()
    }
}

impl<const N: usize> Input for &[stmt::Value; N] {
    fn resolve_arg(&mut self, expr_arg: &stmt::ExprArg, projection: &Projection) -> Value {
        self[expr_arg.position].entry(projection).to_value()
    }
}

impl Input for &[stmt::Value] {
    fn resolve_arg(&mut self, expr_arg: &stmt::ExprArg, projection: &Projection) -> Value {
        self[expr_arg.position].entry(projection).to_value()
    }
}
