use crate::{
    Schema,
    schema::{
        app::{Model, ModelId, ModelRoot},
        db::{Table, TableId},
    },
    stmt::{Expr, ExprArg, ExprContext, ExprReference, Project, Projection, Resolve, Type, Value},
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

    /// Resolves the application model with the given ID, for casts whose
    /// conversion is schema-directed (a `#[document]` embed's record ↔
    /// object conversions).
    ///
    /// Defaults to `None`: inputs without schema access evaluate only
    /// schema-free casts, and a schema-directed cast reaching one fails
    /// loudly at evaluation.
    fn resolve_model(&self, id: ModelId) -> Option<&Model> {
        let _ = id;
        None
    }
}

/// Adapts an [`Input`]'s model resolution to the [`Resolve`] trait, so
/// expression evaluation can hand it to schema-directed casts
/// ([`Type::cast`](crate::stmt::Type::cast)).
pub(crate) struct InputResolve<'a, I: ?Sized>(pub(crate) &'a I);

impl<I: Input + ?Sized> Resolve for InputResolve<'_, I> {
    fn model(&self, id: ModelId) -> Option<&Model> {
        self.0.resolve_model(id)
    }

    fn table(&self, _id: TableId) -> Option<&Table> {
        None
    }

    fn table_for_model(&self, _model: &ModelRoot) -> Option<&Table> {
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
/// that the resolved expression can evaluate to a value of the expected
/// argument type from `tys` (via [`Expr::is_a`]).
pub struct TypedInput<'a, I, T = Schema> {
    cx: ExprContext<'a, T>,
    tys: &'a [Type],
    input: I,
}

impl ConstInput {
    /// Creates a new `ConstInput`.
    pub fn new() -> ConstInput {
        ConstInput {}
    }
}

impl Input for ConstInput {}

impl<'a, I, T> TypedInput<'a, I, T> {
    /// Creates a new `TypedInput` with the given expression context,
    /// expected argument types, and inner input.
    pub fn new(cx: ExprContext<'a, T>, tys: &'a [Type], input: I) -> Self {
        TypedInput { cx, tys, input }
    }
}

impl<I: Input, T: Resolve> Input for TypedInput<'_, I, T> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<Expr> {
        let expr = self.input.resolve_arg(expr_arg, projection)?;

        let mut ty = &self.tys[expr_arg.position];

        for step in projection {
            ty = match ty {
                Type::Record(tys) => &tys[step],
                Type::List(item) => item,
                _ => todo!("ty={ty:#?}"),
            };
        }

        assert!(
            expr.is_a(self.cx.schema(), ty),
            "resolved input cannot evaluate to the requested argument type; expected={ty:#?}; actual={expr:#?}"
        );

        Some(expr)
    }

    fn resolve_model(&self, id: ModelId) -> Option<&Model> {
        self.cx.schema().model(id)
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
