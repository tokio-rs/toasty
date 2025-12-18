use crate::{
    stmt::{Expr, ExprArg, ExprContext, ExprReference, Project, Projection, Resolve, Type, Value},
    Schema,
};

pub trait Input {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        let _ = (expr_arg, projection);
        None
    }

    fn resolve_ref(
        &mut self,
        expr_reference: &ExprReference,
        projection: &Projection,
    ) -> Option<Expr> {
        let _ = (expr_reference, projection);
        None
    }
}

#[derive(Debug, Default)]
pub struct ConstInput {}

pub struct TypedInput<'a, I, T = Schema> {
    cx: ExprContext<'a, T>,
    tys: &'a [Type],
    input: I,
}

impl ConstInput {
    pub fn new() -> ConstInput {
        ConstInput {}
    }
}

impl Input for ConstInput {}

impl<'a, I, T> TypedInput<'a, I, T> {
    pub fn new(cx: ExprContext<'a, T>, tys: &'a [Type], input: I) -> Self {
        TypedInput { cx, tys, input }
    }
}

impl<I: Input, T: Resolve> Input for TypedInput<'_, I, T> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        let expr = self.input.resolve_arg(expr_arg, projection)?;

        if !expr.is_value_null() {
            let actual_ty = self.cx.infer_expr_ty(&expr, &[]);

            let mut ty = &self.tys[expr_arg.position];

            for step in projection {
                ty = match ty {
                    Type::Record(tys) => &tys[step],
                    Type::List(item) => item,
                    _ => todo!("ty={ty:#?}"),
                };
            }

            assert!(actual_ty.is_equivalent(ty), "resolved input did not match requested argument type; expected={ty:#?}; actual={actual_ty:#?}")
        }

        Some(expr)
    }
}

impl Input for &Vec<Value> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        Some(self[expr_arg.position].entry(projection).to_expr())
    }
}

impl<T, const N: usize> Input for [T; N]
where
    for<'a> &'a T: Project,
{
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        (&self[expr_arg.position]).project(projection)
    }
}

impl<T, const N: usize> Input for &[T; N]
where
    for<'a> &'a T: Project,
{
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        (&self[expr_arg.position]).project(projection)
    }
}

impl<T> Input for &[T]
where
    for<'a> &'a T: Project,
{
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        (&self[expr_arg.position]).project(projection)
    }
}
