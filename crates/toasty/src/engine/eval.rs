mod as_expr;
#[cfg(test)]
mod tests;

use as_expr::AsExpr;

use crate::Result;
use toasty_core::{
    schema::Schema,
    stmt::{self, ExprContext},
};

#[derive(Clone, Debug)]
pub(crate) struct Func<T = stmt::Expr> {
    /// Expression arguments
    pub(crate) args: Vec<stmt::Type>,

    /// Expression return type
    pub(crate) ret: stmt::Type,

    /// Expression to evaluate
    expr: T,
}

impl<T: AsExpr> Func<T> {
    pub(crate) fn from_stmt(expr: T, args: Vec<stmt::Type>) -> Self {
        assert!(expr.as_expr().is_eval());
        let ret = ExprContext::new_free().infer_expr_ty(expr.as_expr(), &args);
        Self { args, ret, expr }
    }

    pub(crate) fn from_stmt_typed(expr: T, args: Vec<stmt::Type>, ret: stmt::Type) -> Self {
        Self { args, ret, expr }
    }

    /// Returns true if the function has no inputs
    pub(crate) fn is_const(&self) -> bool {
        self.args.is_empty()
    }

    pub(crate) fn is_identity(&self) -> bool {
        matches!(self.expr.as_expr(), stmt::Expr::Arg(expr_arg) if expr_arg.position == 0)
    }

    /// Evaluates the function against `input`. `schema` directs the
    /// schema-directed conversions (a `#[document]` embed's record ↔ object
    /// casts); every other operation is schema-free.
    pub(crate) fn eval(&self, schema: &Schema, input: impl stmt::Input) -> Result<stmt::Value> {
        use stmt::TypedInput;

        let input = TypedInput::new(stmt::ExprContext::new(schema), &self.args, input);
        self.expr.as_expr().eval(input)
    }

    pub(crate) fn eval_const(&self, schema: &Schema) -> stmt::Value {
        assert!(self.is_const());
        self.eval(schema, stmt::ConstInput::new()).unwrap()
    }

    pub(crate) fn eval_bool(&self, schema: &Schema, input: impl stmt::Input) -> Result<bool> {
        use stmt::TypedInput;

        let input = TypedInput::new(stmt::ExprContext::new(schema), &self.args, input);
        self.expr.as_expr().eval_bool(input)
    }
}

impl Func<stmt::Expr> {
    /// Consumes the function, returning its expression.
    pub(crate) fn into_expr(self) -> stmt::Expr {
        self.expr
    }
}

impl Func<&stmt::Expr> {
    pub(crate) fn try_from_stmt(
        expr: &stmt::Expr,
        args: Vec<stmt::Type>,
    ) -> Option<Func<&stmt::Expr>> {
        if !expr.is_eval() {
            return None;
        }

        let ret = ExprContext::new_free().infer_expr_ty(expr, &args);
        Some(Func::from_stmt_typed(expr, args, ret))
    }
}
