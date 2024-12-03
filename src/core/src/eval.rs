mod convert;
pub use convert::Convert;

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

mod value;

use crate::{
    stmt::{self, BinaryOp, Projection, Value, ValueRecord},
    Result,
};

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
    pub fn convert_stmt(stmt: stmt::Expr, mut convert: impl Convert) -> Option<Expr> {
        Some(Expr::from_stmt_by_ref(stmt, &mut convert))
    }

    pub(crate) fn from_stmt_by_ref(stmt: stmt::Expr, convert: &mut impl Convert) -> Expr {
        match stmt {
            stmt::Expr::Arg(expr) => ExprArg::from_stmt(expr).into(),
            stmt::Expr::And(expr) => ExprAnd::from_stmt(expr, convert).into(),
            stmt::Expr::BinaryOp(expr) => ExprBinaryOp::from_stmt(expr, convert).into(),
            stmt::Expr::Cast(expr) => ExprCast::from_stmt(expr, convert).into(),
            stmt::Expr::Field(expr) => convert.convert_expr_field(expr).unwrap(),
            stmt::Expr::List(expr) => ExprList::from_stmt(expr, convert).into(),
            stmt::Expr::Project(expr) => ExprProject::from_stmt(expr, convert).into(),
            stmt::Expr::Record(expr) => ExprRecord::from_stmt(expr, convert).into(),
            stmt::Expr::Value(expr) => Expr::Value(expr),
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    pub fn eval(&self, mut input: impl Input) -> crate::Result<Value> {
        self.eval_ref(&mut input)
    }

    /// Special case of `eval` where the expression is a constant
    ///
    /// # Panics
    ///
    /// `eval_const` panics if the expression is not constant
    pub fn eval_const(&self) -> Value {
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

    pub(crate) fn eval_ref(&self, input: &mut impl Input) -> Result<Value> {
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
                    Ok(expr_project.projection.resolve_value(&base).clone())
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
}

impl<T: Into<stmt::Expr>> From<T> for Expr {
    fn from(value: T) -> Self {
        Expr::from_stmt_by_ref(value.into(), &mut convert::Const)
    }
}
