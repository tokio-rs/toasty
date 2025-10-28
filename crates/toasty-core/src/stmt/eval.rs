use crate::{
    stmt::{BinaryOp, ConstInput, Expr, Input, Projection, Value},
    Result,
};

impl Expr {
    pub fn eval(&self, mut input: impl Input) -> Result<Value> {
        self.eval_ref(&mut input)
    }

    pub fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        self.eval_ref_bool(&mut input)
    }

    pub fn eval_const(&self) -> Result<Value> {
        self.eval(ConstInput::new())
    }

    fn eval_ref(&self, input: &mut impl Input) -> Result<Value> {
        match self {
            Expr::And(expr_and) => {
                debug_assert!(!expr_and.operands.is_empty());

                for operand in &expr_and.operands {
                    if !operand.eval_ref_bool(input)? {
                        return Ok(false.into());
                    }
                }

                Ok(true.into())
            }
            Expr::Arg(expr_arg) => {
                let Some(expr) = input.resolve_arg(expr_arg, &Projection::identity()) else {
                    anyhow::bail!("failed to resolve argument")
                };
                expr.eval_ref(input)
            }
            Expr::BinaryOp(expr_binary_op) => {
                let lhs = expr_binary_op.lhs.eval_ref(input)?;
                let rhs = expr_binary_op.rhs.eval_ref(input)?;

                match expr_binary_op.op {
                    BinaryOp::Eq => Ok((lhs == rhs).into()),
                    BinaryOp::Ne => Ok((lhs != rhs).into()),
                    _ => todo!("{:#?}", self),
                }
            }
            Expr::Cast(expr_cast) => expr_cast.ty.cast(expr_cast.expr.eval_ref(input)?),
            Expr::IsNull(expr_is_null) => {
                let value = expr_is_null.expr.eval_ref(input)?;
                Ok((value.is_null() != expr_is_null.negate).into())
            }
            Expr::List(exprs) => {
                let mut ret = vec![];

                for expr in &exprs.items {
                    ret.push(expr.eval_ref(input)?);
                }

                Ok(Value::List(ret))
            }
            Expr::Map(expr_map) => {
                let mut base = expr_map.base.eval_ref(input)?;

                let Value::List(ref mut items) = &mut base else {
                    todo!("base={base:#?}")
                };

                for item in items.iter_mut() {
                    let i = item.take();
                    *item = expr_map.map.eval_ref(&mut &[i])?;
                }

                Ok(base)
            }
            Expr::Project(expr_project) => match &*expr_project.base {
                Expr::Arg(expr_arg) => {
                    let Some(expr) = input.resolve_arg(expr_arg, &expr_project.projection) else {
                        anyhow::bail!("failed to resolve argument")
                    };

                    expr.eval_ref(input)
                }
                Expr::Reference(expr_reference) => {
                    let Some(expr) = input.resolve_ref(expr_reference, &expr_project.projection)
                    else {
                        anyhow::bail!("failed to resolve reference")
                    };

                    expr.eval_ref(input)
                }
                _ => {
                    let base = expr_project.base.eval_ref(input)?;
                    Ok(base.entry(&expr_project.projection).to_value())
                }
            },
            Expr::Record(expr_record) => {
                let mut ret = Vec::with_capacity(expr_record.len());

                for expr in &expr_record.fields {
                    ret.push(expr.eval_ref(input)?);
                }

                Ok(Value::record_from_vec(ret))
            }
            Expr::Reference(expr_reference) => {
                let Some(expr) = input.resolve_ref(expr_reference, &Projection::identity()) else {
                    anyhow::bail!("failed to resolve reference")
                };

                expr.eval_ref(input)
            }
            Expr::Value(value) => Ok(value.clone()),
            Expr::DecodeEnum(expr, ty, variant) => {
                let Value::String(base) = expr.eval_ref(input)? else {
                    todo!()
                };
                let (decoded_variant, rest) = base.split_once("#").unwrap();
                let decoded_variant: usize = decoded_variant.parse()?;

                if decoded_variant != *variant {
                    todo!("error; decoded={decoded_variant:#?}; expr={expr:#?}; ty={ty:#?}; variant={variant:#?}");
                }

                ty.cast(rest.into())
            }
            _ => todo!("expr={self:#?}"),
        }
    }

    fn eval_ref_bool(&self, input: &mut impl Input) -> Result<bool> {
        match self.eval_ref(input)? {
            Value::Bool(ret) => Ok(ret),
            _ => anyhow::bail!("not boolean value"),
        }
    }
}
