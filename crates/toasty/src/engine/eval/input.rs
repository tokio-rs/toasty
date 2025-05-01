use super::*;

use stmt::{self, Projection, Value};

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &stmt::ExprArg, projection: &Projection) -> Value;
}

pub(super) struct TypedInput<'a, I> {
    input: &'a mut I,
    tys: &'a [stmt::Type],
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

impl<'a, I> TypedInput<'a, I> {
    pub(super) fn new(input: &'a mut I, tys: &'a [stmt::Type]) -> Self {
        TypedInput { input, tys }
    }
}

impl<I: Input> Input for TypedInput<'_, I> {
    fn resolve_arg(
        &mut self,
        expr_arg: &stmt::ExprArg,
        projection: &stmt::Projection,
    ) -> stmt::Value {
        let value = self.input.resolve_arg(expr_arg, projection);

        if !value.is_null() {
            let mut ty = &self.tys[expr_arg.position];

            for step in projection {
                ty = match ty {
                    stmt::Type::Record(tys) => &tys[step],
                    _ => todo!("ty={ty:#?}"),
                };
            }

            assert!(value.is_a(ty), "value={value:#?}; ty={ty:#?};");
        }

        value
    }
}
