use super::{Expr, Type};

/// A type cast expression.
///
/// Converts an expression's value to a different type.
///
/// # Examples
///
/// ```text
/// cast(x, i64)     // cast `x` to `i64`
/// cast(y, string)  // cast `y` to `string`
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprCast {
    /// The expression to cast.
    pub expr: Box<Expr>,

    /// The source type, when the value alone cannot direct the conversion.
    ///
    /// Most casts are directed by the target type and the value's own shape,
    /// and leave this `None`. The exception is a `#[document]` column's
    /// lowering cast (`Type::Model` → `Type::Object`): the structural target
    /// does not name the embedded model and a positional `Value::Record` is
    /// not self-describing, so the cast carries the model-level source type
    /// to resolve the embed's field names.
    pub from: Option<Type>,

    /// The target type.
    pub ty: Type,
}

impl Expr {
    /// Creates a type cast expression that converts `expr` to the target type.
    pub fn cast(expr: impl Into<Self>, ty: impl Into<Type>) -> Self {
        ExprCast {
            expr: Box::new(expr.into()),
            from: None,
            ty: ty.into(),
        }
        .into()
    }

    /// Creates a type cast expression whose conversion is directed by the
    /// source type as well as the target type. See [`ExprCast::from`].
    pub fn cast_from(expr: impl Into<Self>, from: impl Into<Type>, ty: impl Into<Type>) -> Self {
        ExprCast {
            expr: Box::new(expr.into()),
            from: Some(from.into()),
            ty: ty.into(),
        }
        .into()
    }

    /// Returns `true` if this expression is a type cast.
    pub fn is_cast(&self) -> bool {
        matches!(self, Self::Cast(_))
    }
}

impl From<ExprCast> for Expr {
    fn from(value: ExprCast) -> Self {
        Self::Cast(value)
    }
}
