use crate::schema::db;

use super::{Expr, ExprFunc};

/// The PostgreSQL `unnest` set-returning function.
///
/// Each argument is an array expression. PostgreSQL emits one output row for
/// each array position and one output column for each argument.
#[derive(Clone, Debug, PartialEq)]
pub struct FuncUnnest {
    /// The arrays expanded into rows.
    pub args: Vec<FuncUnnestArg>,
}

/// A typed argument to [`FuncUnnest`].
#[derive(Clone, Debug, PartialEq)]
pub struct FuncUnnestArg {
    /// The array expression.
    pub expr: Expr,

    /// The database type of each array element.
    pub elem_ty: db::Type,
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
