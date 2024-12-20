mod as_expr;
use as_expr::AsExpr;

mod convert;
pub(crate) use convert::Convert;

mod input;
pub(crate) use input::Input;

use super::*;

pub(crate) struct Eval<T = stmt::Expr> {
    /// Expression arguments
    pub args: Vec<stmt::Type>,

    /// Expression return type
    pub ret: stmt::Type,

    /// Expression to evaluate
    expr: T,
}

impl<T: AsExpr> Eval<T> {
    pub fn from_stmt_unchecked(expr: T, args: Vec<stmt::Type>, ret: stmt::Type) -> Eval<T> {
        Eval { args, ret, expr }
    }

    pub fn eval(&self, mut input: impl Input) -> Result<stmt::Value> {
        use input::TypedInput;

        let mut input = TypedInput::new(&mut input, &self.args);
        eval(self.expr.as_expr(), &mut input)
    }
}

impl Eval<&stmt::Expr> {
    pub fn try_from_stmt(args: Vec<stmt::Type>, expr: &stmt::Expr) -> Option<Eval<&stmt::Expr>> {
        if !verify_eval(expr) {
            return None;
        }

        let ret = infer_ty(expr, &args);
        Some(Eval::from_stmt_unchecked(expr, args, ret))
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
        Value(value) => Ok(value.clone()),
        BinaryOp(expr_binary_op) => {
            let lhs = eval(&*expr_binary_op.lhs, input)?;
            let rhs = eval(&*expr_binary_op.rhs, input)?;

            match expr_binary_op.op {
                stmt::BinaryOp::Eq => Ok((lhs == rhs).into()),
                stmt::BinaryOp::Ne => Ok((lhs != rhs).into()),
                _ => todo!("{:#?}", expr),
            }
        }
        Cast(expr_cast) => expr_cast.ty.cast(eval(&*expr_cast.expr, input)?),
        Project(expr_project) => {
            if let Arg(expr_arg) = &*expr_project.base {
                Ok(input.resolve_arg(expr_arg, &expr_project.projection))
            } else {
                let base = eval(&*expr_project.base, input)?;
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
        List(exprs) => {
            let mut ret = vec![];

            for expr in &exprs.items {
                ret.push(eval(expr, input)?);
            }

            Ok(stmt::Value::List(ret))
        }
        Map(expr_map) => {
            let mut base = eval(&*expr_map.base, input)?;

            let stmt::Value::List(ref mut items) = &mut base else {
                todo!("base={base:#?}")
            };

            for item in items.iter_mut() {
                let mut i = item.take();
                *item = eval(&*expr_map.map, &mut &[i])?;
            }

            Ok(base)
        }
        DecodeEnum(expr, ty, variant) => {
            let stmt::Value::String(base) = eval(&*expr, input)? else {
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

fn verify_eval(expr: &stmt::Expr) -> bool {
    use stmt::Expr::*;

    match expr {
        Arg(_) => true,
        And(expr_and) => expr_and.operands.iter().all(verify_eval),
        BinaryOp(expr) => verify_eval(&*expr.lhs) && verify_eval(&*expr.rhs),
        Cast(expr) => verify_eval(&*expr.expr),
        Column(_) => false,
        Field(_) => false,
        List(expr) => expr.items.iter().all(verify_eval),
        Map(expr) => verify_eval(&*expr.base) && verify_eval(&*expr.map),
        Project(expr) => verify_eval(&*expr.base),
        Record(expr) => expr.fields.iter().all(verify_eval),
        Value(_) => true,
        DecodeEnum(expr, _, _) => verify_eval(&*expr),
        _ => todo!("expr={expr:#?}"),
    }
}

fn infer_ty(expr: &stmt::Expr, args: &[stmt::Type]) -> stmt::Type {
    use stmt::Expr::*;

    match expr {
        And(_) => stmt::Type::Bool,
        Arg(expr_arg) => args[expr_arg.position].clone(),
        Value(value) => value.ty(),
        BinaryOp(_) => stmt::Type::Bool,
        Cast(expr_cast) => expr_cast.ty.clone(),
        List(_) => todo!("{expr:#?}"),
        Map(expr_map) => {
            let base = infer_ty(&*expr_map.base, args);
            infer_ty(&*expr_map.map, &[base])
        }
        Record(expr_record) => {
            let mut fields = Vec::with_capacity(expr_record.len());

            for expr in &expr_record.fields {
                fields.push(infer_ty(expr, args));
            }

            stmt::Type::Record(fields)
        }
        _ => todo!("expr={expr:#?}"),
    }
}
