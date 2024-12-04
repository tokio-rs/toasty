mod convert;
pub use convert::Convert;

mod expr;
pub use expr::Expr;

mod expr_and;
pub use expr_and::ExprAnd;

mod expr_arg;
pub use expr_arg::ExprArg;

mod expr_binary_op;
pub use expr_binary_op::ExprBinaryOp;

mod expr_cast;
pub use expr_cast::ExprCast;

mod expr_list;
pub use expr_list::ExprList;

mod expr_map;
pub use expr_map::ExprMap;

mod expr_or;
pub use expr_or::ExprOr;

mod expr_project;
pub use expr_project::ExprProject;

mod expr_record;
pub use expr_record::ExprRecord;

mod input;
pub use input::{const_input, Input};

use crate::{stmt, Result};

#[derive(Debug, Clone)]
pub struct Eval {
    /// Argument types
    pub args: Vec<stmt::Type>,

    /// Expression to evaluate
    pub expr: Expr,
}

impl Eval {
    pub fn eval(&self, mut input: impl Input) -> crate::Result<stmt::Value> {
        self.expr.eval_ref(&mut input)
    }

    /// Special case of `eval` where the expression is a constant
    ///
    /// # Panics
    ///
    /// `eval_const` panics if the expression is not constant
    pub fn eval_const(&self) -> stmt::Value {
        assert!(self.args.is_empty());
        self.expr.eval_ref(&mut const_input()).unwrap()
    }

    pub fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        self.expr.eval_bool_ref(&mut input)
    }
}
