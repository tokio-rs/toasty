use super::*;

pub trait Input<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt>;

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr<'stmt>;
}

pub struct Args<T>(T);

impl<'stmt> Input<'stmt> for &Value<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        let mut ret = &**self;

        for step in projection {
            ret = match ret {
                Value::Record(record) => &record[step.into_usize()],
                _ => todo!(),
            };
        }

        Expr::Value(ret.clone())
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Expr<'stmt> {
        panic!("no argument source provided")
    }
}

impl<'stmt> Input<'stmt> for &ExprRecord<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        resolve::resolve(&**self, projection).unwrap().clone()
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Expr<'stmt> {
        panic!("no argument source provided")
    }
}

impl<'stmt> Input<'stmt> for &[Expr<'stmt>] {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        resolve::resolve(&**self, projection).unwrap().clone()
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Expr<'stmt> {
        panic!("no argument source provided")
    }
}

pub fn args<T>(input: T) -> Args<T> {
    Args(input)
}

impl<'stmt> Input<'stmt> for Args<&[Value<'stmt>]> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        Expr::Project(ExprProject {
            base: ProjectBase::ExprSelf,
            projection: projection.clone(),
        })
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr<'stmt> {
        Expr::Value(self.0[expr_arg.position].clone())
    }
}

impl<'stmt> Input<'stmt> for Args<&[Expr<'stmt>]> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        Expr::Project(ExprProject {
            base: ProjectBase::ExprSelf,
            projection: projection.clone(),
        })
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr<'stmt> {
        self.0[expr_arg.position].clone()
    }
}
