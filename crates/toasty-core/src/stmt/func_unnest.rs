use super::{Expr, ExprFunc};

/// The PostgreSQL `unnest` set-returning function.
///
/// The argument is an array expression. PostgreSQL emits one output row for
/// each array element.
#[derive(Clone, Debug, PartialEq)]
pub struct FuncUnnest {
    /// The array expression.
    pub arg: Box<Expr>,
}

impl From<FuncUnnest> for ExprFunc {
    fn from(value: FuncUnnest) -> Self {
        Self::Unnest(value)
    }
}

impl From<FuncUnnest> for Expr {
    fn from(value: FuncUnnest) -> Self {
        Self::Func(value.into())
    }
}
