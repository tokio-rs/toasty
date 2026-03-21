use crate::{
    stmt::{Expr, ExprArg, ExprContext, ExprReference, Project, Projection, Resolve, Type, Value},
    Schema,
};

/// Provides runtime argument and reference resolution for expression
/// evaluation and substitution.
///
/// During expression evaluation, `Arg` and `Reference` nodes are resolved
/// by calling methods on an `Input` implementation. The default methods
/// return `None` (unresolved).
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::{ConstInput, Input};
///
/// // ConstInput resolves nothing -- suitable for expressions with no
/// // external arguments.
/// let mut input = ConstInput::new();
/// ```
pub trait Input {
    /// Resolves an argument expression at the given projection.
    ///
    /// Returns `Some(expr)` if the argument can be resolved, or `None`
    /// if it cannot.
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        let _ = (expr_arg, projection);
        None
    }

    /// Resolves a reference expression at the given projection.
    ///
    /// Returns `Some(expr)` if the reference can be resolved, or `None`
    /// if it cannot.
    fn resolve_ref(
        &mut self,
        expr_reference: &ExprReference,
        projection: &Projection,
    ) -> Option<Expr> {
        let _ = (expr_reference, projection);
        None
    }
}

/// An [`Input`] implementation that resolves nothing.
///
/// Use `ConstInput` when evaluating expressions that contain no external
/// arguments or references (i.e., constant expressions).
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::{ConstInput, Value, Expr};
///
/// let expr = Expr::from(Value::from(42_i64));
/// let result = expr.eval(ConstInput::new()).unwrap();
/// assert_eq!(result, Value::from(42_i64));
/// ```
#[derive(Debug, Default)]
pub struct ConstInput {}

/// An [`Input`] wrapper that validates resolved argument types against
/// expected types at resolution time.
///
/// `TypedInput` delegates resolution to an inner `Input` and then checks
/// that the resolved expression's inferred type is a subtype of the
/// expected argument type from `tys`.
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

            assert!(actual_ty.is_subtype_of(ty), "resolved input did not match requested argument type; expected={ty:#?}; actual={actual_ty:#?}")
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
