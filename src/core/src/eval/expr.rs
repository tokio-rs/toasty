use super::*;

use stmt::{BinaryOp, Projection, Value};

#[derive(Debug, Clone)]
pub enum Expr {
    And(ExprAnd),
    Arg(ExprArg),
    BinaryOp(ExprBinaryOp),
    Cast(ExprCast),
    List(ExprList),
    Map(ExprMap),
    Project(ExprProject),
    Record(ExprRecord),
    Value(Value),
    // Hax
    DecodeEnum(Box<Expr>, stmt::Type),
}

impl Expr {
    pub fn null() -> Expr {
        Expr::Value(Value::Null)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Expr::Value(Value::Null))
    }

    pub fn from_stmt(stmt: stmt::Expr) -> Expr {
        Expr::try_convert_from_stmt(stmt, convert::ConstExpr).expect("non-const expr")
    }

    pub fn try_convert_from_stmt(stmt: stmt::Expr, mut convert: impl Convert) -> Option<Expr> {
        Some(Expr::from_stmt_by_ref(stmt, &mut convert))
    }

    pub(crate) fn from_stmt_by_ref(stmt: stmt::Expr, convert: &mut impl Convert) -> Expr {
        match stmt {
            stmt::Expr::Arg(expr) => ExprArg::from_stmt(expr).into(),
            stmt::Expr::And(expr) => ExprAnd::from_stmt(expr, convert).into(),
            stmt::Expr::BinaryOp(expr) => ExprBinaryOp::from_stmt(expr, convert).into(),
            stmt::Expr::Cast(expr) => ExprCast::from_stmt(expr, convert).into(),
            stmt::Expr::Field(expr) => convert.convert_expr_field(expr),
            stmt::Expr::List(expr) => ExprList::from_stmt(expr, convert).into(),
            stmt::Expr::Project(expr) => ExprProject::from_stmt(expr, convert).into(),
            stmt::Expr::Record(expr) => ExprRecord::from_stmt(expr, convert).into(),
            stmt::Expr::Value(expr) => Expr::Value(expr),
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    // pub fn eval(&self, mut input: impl Input) -> crate::Result<stmt::Value> {
    //     self.eval_ref(&mut input)
    // }

    /// Special case of `eval` where the expression is a constant
    ///
    /// # Panics
    ///
    /// `eval_const` panics if the expression is not constant
    pub fn eval_const(&self) -> stmt::Value {
        self.eval_ref(&mut const_input()).unwrap()
    }

    pub fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        self.eval_bool_ref(&mut input)
    }

    pub(crate) fn eval_bool_ref(&self, input: &mut impl Input) -> Result<bool> {
        match self.eval_ref(input)? {
            Value::Bool(ret) => Ok(ret),
            _ => todo!(),
        }
    }

    pub(super) fn eval_ref(&self, input: &mut impl Input) -> Result<Value> {
        match self {
            Expr::And(expr_and) => {
                debug_assert!(!expr_and.operands.is_empty());

                for operand in &expr_and.operands {
                    if !operand.eval_bool_ref(input)? {
                        return Ok(false.into());
                    }
                }

                Ok(true.into())
            }
            Expr::Arg(expr_arg) => Ok(input.resolve_arg(expr_arg, &Projection::identity())),
            Expr::Value(value) => Ok(value.clone()),
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
            /*
            Expr::Enum(expr_enum) => Ok(ValueEnum {
                variant: expr_enum.variant,
                fields: expr_enum.fields.eval_ref(input)?,
            }
            .into()),
            */
            Expr::Project(expr_project) => {
                if let Expr::Arg(expr_arg) = &*expr_project.base {
                    Ok(input.resolve_arg(expr_arg, &expr_project.projection))
                } else {
                    let base = expr_project.base.eval_ref(input)?;
                    Ok(base.entry(&expr_project.projection).to_value())
                }
            }
            Expr::Record(expr_record) => Ok(expr_record.eval_ref(input)?.into()),
            Expr::List(exprs) => {
                let mut applied = vec![];

                for expr in &exprs.items {
                    applied.push(expr.eval_ref(input)?);
                }

                Ok(Value::List(applied))
            }
            Expr::Map(expr_map) => {
                /*
                let base = expr_map.base.eval_ref(input)?;
                expr_map.map.eval(&base)
                */
                todo!()
            }
            Expr::DecodeEnum(expr, ty) => {
                let Value::String(base) = expr.eval_ref(input)? else {
                    todo!()
                };
                let (variant, rest) = base.split_once("#").unwrap();
                ty.cast(rest.into())
            }
            _ => todo!("expr={self:#?}"),
        }
    }

    pub(crate) fn ty(&self, args: &[stmt::Type]) -> stmt::Type {
        match self {
            Expr::And(_) => stmt::Type::Bool,
            Expr::Arg(arg) => args[arg.position].clone(),
            Expr::BinaryOp(_) => stmt::Type::Bool,
            Expr::Cast(e) => e.ty.clone(),
            Expr::List(_) => todo!("{self:#?}"),
            Expr::Map(e) => {
                let base = e.base.ty(args);
                e.map.ty(&[base])
            }
            Expr::Project(e) => todo!("{self:#?}"),
            Expr::Record(e) => {
                stmt::Type::Record(e.fields.iter().map(|field| field.ty(args)).collect())
            }
            Expr::Value(value) => value.ty(),
            _ => todo!("expr={self:#?}"),
        }
    }
}
