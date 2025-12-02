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

    /// The target type.
    pub ty: Type,
}

impl Expr {
    pub fn cast(expr: impl Into<Self>, ty: impl Into<Type>) -> Self {
        ExprCast {
            expr: Box::new(expr.into()),
            ty: ty.into(),
        }
        .into()
    }

    pub fn is_cast(&self) -> bool {
        matches!(self, Self::Cast(_))
    }
}

impl From<ExprCast> for Expr {
    fn from(value: ExprCast) -> Self {
        Self::Cast(value)
    }
}
