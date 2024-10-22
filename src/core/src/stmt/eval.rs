use super::*;

pub trait Input<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Value<'stmt>;

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Value<'stmt>;
}

#[derive(Debug)]
pub struct Args<T>(T);

pub fn args<T>(input: T) -> Args<T> {
    Args(input)
}

pub fn const_input<'stmt>() -> impl Input<'stmt> {
    struct Unused;

    impl<'stmt> Input<'stmt> for Unused {
        fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Value<'stmt> {
            panic!("no input provided")
        }

        fn resolve_self_projection(&mut self, _projection: &Projection) -> Value<'stmt> {
            panic!("no input provided")
        }
    }

    Unused
}

impl<'stmt> Input<'stmt> for &Record<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Value<'stmt> {
        // TODO: dedup w/ below

        let [first, rest @ ..] = projection.as_slice() else {
            todo!()
        };

        let mut ret = &self[first.into_usize()];

        for step in rest {
            ret = match ret {
                Value::Record(record) => &record[step.into_usize()],
                Value::Enum(value_enum) => &value_enum.fields[0],
                _ => todo!(
                    "ret={:#?}; step={:#?}; projection={:#?}",
                    ret,
                    step,
                    projection
                ),
            };
        }

        ret.clone()
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Value<'stmt> {
        panic!("no argument source provided")
    }
}

impl<'stmt> Input<'stmt> for &[Value<'stmt>] {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Value<'stmt> {
        let [first, rest @ ..] = projection.as_slice() else {
            todo!()
        };

        let mut ret = &self[first.into_usize()];

        for step in rest {
            ret = match ret {
                Value::Record(record) => &record[step.into_usize()],
                Value::Enum(value_enum) => &value_enum.fields[0],
                _ => todo!(
                    "ret={:#?}; step={:#?}; projection={:#?}",
                    ret,
                    step,
                    projection
                ),
            };
        }

        ret.clone()
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Value<'stmt> {
        panic!("no argument source provided")
    }
}

impl<'stmt> Input<'stmt> for &Value<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Value<'stmt> {
        let mut ret = *self;

        for step in projection {
            ret = match ret {
                Value::Record(record) => &record[step.into_usize()],
                Value::Enum(value_enum) => &value_enum.fields[0],
                _ => todo!(),
            };
        }

        ret.clone()
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Value<'stmt> {
        panic!("no argument source provided; expr_arg={:#?}", expr_arg)
    }
}

impl<'stmt> Input<'stmt> for Args<&[Value<'stmt>]> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Value<'stmt> {
        panic!(
            "no `expr_self` provided; input={:#?}; projection={:#?}",
            self, projection
        );
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Value<'stmt> {
        self.0[expr_arg.position].clone()
    }
}
