mod as_expr;
use as_expr::AsExpr;

mod convert;
pub(crate) use convert::Convert;

mod input;
pub(crate) use input::Input;

use super::*;
use crate::engine::ty;

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
        let ret = ty::infer_eval_expr_ty(expr.as_expr(), &args);
        Self { args, ret, expr }
    }

    pub(crate) fn from_stmt_unchecked(expr: T, args: Vec<stmt::Type>, ret: stmt::Type) -> Self {
        Self { args, ret, expr }
    }

    /// Returns true if the function has no inputs
    pub(crate) fn is_const(&self) -> bool {
        self.args.is_empty()
    }

    pub(crate) fn is_identity(&self) -> bool {
        matches!(self.expr.as_expr(), stmt::Expr::Arg(expr_arg) if expr_arg.position == 0)
    }

    pub(crate) fn eval(&self, mut input: impl Input) -> Result<stmt::Value> {
        use input::TypedInput;

        let mut input = TypedInput::new(&mut input, &self.args);
        eval(self.expr.as_expr(), &mut input)
    }

    pub(crate) fn eval_const(&self) -> stmt::Value {
        assert!(self.is_const());
        eval(self.expr.as_expr(), &mut input::const_input()).unwrap()
    }

    pub(crate) fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        use input::TypedInput;

        let mut input = TypedInput::new(&mut input, &self.args);
        eval_bool(self.expr.as_expr(), &mut input)
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

        let ret = ty::infer_eval_expr_ty(&expr, &args);
        Some(Self::from_stmt_unchecked(expr, args, ret))
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

        let ret = ty::infer_eval_expr_ty(expr, &args);
        Some(Func::from_stmt_unchecked(expr, args, ret))
    }
}

fn eval_bool(expr: &stmt::Expr, input: &mut impl Input) -> Result<bool> {
    match eval(expr, input)? {
        stmt::Value::Bool(ret) => Ok(ret),
        _ => todo!(),
    }
}

fn eval(expr: &stmt::Expr, input: &mut impl Input) -> Result<stmt::Value> {
    use stmt::Expr::*;

    match expr {
        And(expr_and) => {
            debug_assert!(!expr_and.operands.is_empty());

            for operand in &expr_and.operands {
                if !eval_bool(operand, input)? {
                    return Ok(false.into());
                }
            }

            Ok(true.into())
        }
        Arg(expr_arg) => Ok(input.resolve_arg(expr_arg, &stmt::Projection::identity())),
        BinaryOp(expr_binary_op) => {
            let lhs = eval(&expr_binary_op.lhs, input)?;
            let rhs = eval(&expr_binary_op.rhs, input)?;

            match expr_binary_op.op {
                stmt::BinaryOp::Eq => Ok((lhs == rhs).into()),
                stmt::BinaryOp::Ne => Ok((lhs != rhs).into()),
                _ => todo!("{:#?}", expr),
            }
        }
        Cast(expr_cast) => expr_cast.ty.cast(eval(&expr_cast.expr, input)?),
        IsNull(expr_is_null) => {
            let value = eval(&expr_is_null.expr, input)?;
            Ok((value.is_null() != expr_is_null.negate).into())
        }
        List(exprs) => {
            let mut ret = vec![];

            for expr in &exprs.items {
                ret.push(eval(expr, input)?);
            }

            Ok(stmt::Value::List(ret))
        }
        Map(expr_map) => {
            let mut base = eval(&expr_map.base, input)?;

            let stmt::Value::List(ref mut items) = &mut base else {
                todo!("base={base:#?}")
            };

            for item in items.iter_mut() {
                let i = item.take();
                *item = eval(&expr_map.map, &mut &[i])?;
            }

            Ok(base)
        }
        Project(expr_project) => {
            if let Arg(expr_arg) = &*expr_project.base {
                Ok(input.resolve_arg(expr_arg, &expr_project.projection))
            } else {
                let base = eval(&expr_project.base, input)?;
                Ok(base.entry(&expr_project.projection).to_value())
            }
        }
        Record(expr_record) => {
            let mut ret = Vec::with_capacity(expr_record.len());

            for expr in &expr_record.fields {
                ret.push(eval(expr, input)?);
            }

            Ok(stmt::Value::record_from_vec(ret))
        }
        Value(value) => Ok(value.clone()),
        DecodeEnum(expr, ty, variant) => {
            let stmt::Value::String(base) = eval(expr, input)? else {
                todo!()
            };
            let (decoded_variant, rest) = base.split_once("#").unwrap();
            let decoded_variant: usize = decoded_variant.parse()?;

            if decoded_variant != *variant {
                todo!("error");
            }

            ty.cast(rest.into())
        }
        _ => todo!("expr={expr:#?}"),
    }
}

fn verify_expr(expr: &stmt::Expr) -> bool {
    use stmt::Expr::*;

    match expr {
        Arg(_) => true,
        And(expr_and) => expr_and.operands.iter().all(verify_expr),
        BinaryOp(expr) => verify_expr(&expr.lhs) && verify_expr(&expr.rhs),
        Cast(expr) => verify_expr(&expr.expr),
        Column(_) => false,
        Field(_) => false,
        List(expr) => expr.items.iter().all(verify_expr),
        Map(expr) => verify_expr(&expr.base) && verify_expr(&expr.map),
        Project(expr) => verify_expr(&expr.base),
        Record(expr) => expr.fields.iter().all(verify_expr),
        Value(_) => true,
        DecodeEnum(expr, _, _) => verify_expr(expr),
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
        Column(e) => {
            let Some(e) = convert.convert_expr_column(e) else {
                return false;
            };
            *expr = e;
            convert_and_verify_expr(expr, convert)
        }
        Field(e) => {
            let Some(e) = convert.convert_expr_field(e) else {
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
