mod as_expr;
use as_expr::AsExpr;

mod convert;
pub(crate) use convert::Convert;

use crate::Result;
use toasty_core::stmt::{self, ExprContext};

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
        assert!(verify_expr(expr.as_expr()));
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

    pub(crate) fn eval(&self, input: impl stmt::Input) -> Result<stmt::Value> {
        use stmt::TypedInput;

        let input = TypedInput::new(stmt::ExprContext::new_free(), &self.args, input);
        self.expr.as_expr().eval(input)
    }

    pub(crate) fn eval_const(&self) -> stmt::Value {
        assert!(self.is_const());
        self.expr.as_expr().eval_const().unwrap()
    }

    pub(crate) fn eval_bool(&self, input: impl stmt::Input) -> Result<bool> {
        use stmt::TypedInput;

        let input = TypedInput::new(stmt::ExprContext::new_free(), &self.args, input);
        self.expr.as_expr().eval_bool(input)
    }
}

impl Func<stmt::Expr> {
    /// Returns the identity function for the given type
    pub(crate) fn identity(ty: stmt::Type) -> Self {
        Self {
            args: vec![ty.clone()],
            ret: ty,
            expr: stmt::Expr::arg(0),
        }
    }

    pub fn try_convert_from_stmt(
        mut expr: stmt::Expr,
        args: Vec<stmt::Type>,
        mut convert: impl Convert,
    ) -> Option<Self> {
        if !convert_and_verify_expr(&mut expr, &mut convert) {
            return None;
        }

        let ret = ExprContext::new_free().infer_expr_ty(&expr, &args);
        Some(Self::from_stmt_typed(expr, args, ret))
    }
}

impl Func<&stmt::Expr> {
    pub(crate) fn try_from_stmt(
        expr: &stmt::Expr,
        args: Vec<stmt::Type>,
    ) -> Option<Func<&stmt::Expr>> {
        if !verify_expr(expr) {
            return None;
        }

        let ret = ExprContext::new_free().infer_expr_ty(expr, &args);
        Some(Func::from_stmt_typed(expr, args, ret))
    }
}

fn verify_expr(expr: &stmt::Expr) -> bool {
    use stmt::Expr::*;

    match expr {
        Arg(_) => true,
        And(expr_and) => expr_and.operands.iter().all(verify_expr),
        BinaryOp(expr) => verify_expr(&expr.lhs) && verify_expr(&expr.rhs),
        Cast(expr) => verify_expr(&expr.expr),
        DecodeEnum(expr, _, _) => verify_expr(expr),
        IsNull(expr) => verify_expr(&expr.expr),
        List(expr) => expr.items.iter().all(verify_expr),
        Map(expr) => verify_expr(&expr.base) && verify_expr(&expr.map),
        Project(expr) => verify_expr(&expr.base),
        Record(expr) => expr.fields.iter().all(verify_expr),
        Reference(_) => false,
        Value(_) => true,
        _ => todo!("expr={expr:#?}"),
    }
}

fn convert_and_verify_expr(expr: &mut stmt::Expr, convert: &mut impl Convert) -> bool {
    use stmt::Expr::*;

    match expr {
        Arg(_) => true,
        And(expr_and) => expr_and
            .operands
            .iter_mut()
            .all(|e| convert_and_verify_expr(e, convert)),
        BinaryOp(expr) => {
            convert_and_verify_expr(&mut expr.lhs, convert)
                && convert_and_verify_expr(&mut expr.rhs, convert)
        }
        Cast(expr) => convert_and_verify_expr(&mut expr.expr, convert),
        Reference(expr_reference) => {
            let Some(e) = convert.convert_expr_reference(expr_reference) else {
                return false;
            };
            *expr = e;
            convert_and_verify_expr(expr, convert)
        }
        IsNull(e) => convert_and_verify_expr(&mut e.expr, convert),
        List(expr) => expr
            .items
            .iter_mut()
            .all(|e| convert_and_verify_expr(e, convert)),
        Map(expr) => convert_and_verify_expr(&mut expr.base, convert) && verify_expr(&expr.map),
        Project(expr) => convert_and_verify_expr(&mut expr.base, convert),
        Record(expr) => expr
            .fields
            .iter_mut()
            .all(|e| convert_and_verify_expr(e, convert)),
        Value(_) => true,
        DecodeEnum(expr, _, _) => convert_and_verify_expr(&mut *expr, convert),
        _ => todo!("expr={expr:#?}"),
    }
}
