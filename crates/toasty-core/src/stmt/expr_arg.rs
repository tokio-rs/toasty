use super::Expr;
use crate::stmt;

/// A positional argument placeholder.
///
/// Represents a reference to an input value by position. During substitution,
/// `arg(n)` is replaced with the nth value from the input.
///
/// # Examples
///
/// ```text
/// arg(0)  // refers to the first input value
/// arg(1)  // refers to the second input value
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ExprArg {
    /// The zero-based position of the argument.
    pub position: usize,
}

impl Expr {
    pub fn arg(expr_arg: impl Into<ExprArg>) -> Self {
        Self::Arg(expr_arg.into())
    }

    pub fn arg_project(
        expr_arg: impl Into<ExprArg>,
        projection: impl Into<stmt::Projection>,
    ) -> Self {
        Self::project(Self::arg(expr_arg), projection)
    }
}

impl ExprArg {
    pub fn new(position: usize) -> ExprArg {
        ExprArg { position }
    }
}

impl From<usize> for ExprArg {
    fn from(value: usize) -> Self {
        Self { position: value }
    }
}

impl From<ExprArg> for Expr {
    fn from(value: ExprArg) -> Self {
        Self::Arg(value)
    }
}
