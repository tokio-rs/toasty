mod expr_and;
pub use expr_and::ExprAnd;

mod expr_binary_op;
pub use expr_binary_op::ExprBinaryOp;

mod expr_list;
pub use expr_list::ExprList;

mod expr_map;
pub use expr_map::ExprMap;

mod expr_or;
pub use expr_or::ExprOr;

mod expr_project;
pub use expr_project::{ExprProject, ProjectBase};

mod expr_record;
pub use expr_record::ExprRecord;

mod value;

use crate::{
    stmt::{self, eval, BinaryOp, Record, Value},
    Result,
};

#[derive(Debug, Clone)]
pub enum Expr<'stmt> {
    And(ExprAnd<'stmt>),
    BinaryOp(ExprBinaryOp<'stmt>),
    List(ExprList<'stmt>),
    Map(ExprMap<'stmt>),
    Project(ExprProject<'stmt>),
    Record(ExprRecord<'stmt>),
    Value(Value<'stmt>),
}

impl<'stmt> Expr<'stmt> {
    pub fn from_stmt(stmt: stmt::Expr<'stmt>) -> Expr<'stmt> {
        match stmt {
            stmt::Expr::And(expr) => ExprAnd::from_stmt(expr).into(),
            stmt::Expr::BinaryOp(expr) => ExprBinaryOp::from_stmt(expr).into(),
            stmt::Expr::List(expr) => ExprList::from_stmt(expr).into(),
            stmt::Expr::Project(expr) => ExprProject::from_stmt(expr).into(),
            stmt::Expr::Record(expr) => ExprRecord::from_stmt(expr).into(),
            stmt::Expr::Value(expr) => Expr::Value(expr),
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    pub fn eval(&self, mut input: impl eval::Input<'stmt>) -> crate::Result<Value<'stmt>> {
        self.eval_ref(&mut input)
    }

    /// Special case of `eval` where the expression is a constant
    ///
    /// # Panics
    ///
    /// `eval_const` panics if the expression is not constant
    pub fn eval_const(&self) -> Value<'stmt> {
        self.eval_ref(&mut eval::const_input()).unwrap()
    }

    pub fn eval_bool(&self, mut input: impl eval::Input<'stmt>) -> Result<bool> {
        self.eval_bool_ref(&mut input)
    }

    pub(crate) fn eval_bool_ref(&self, input: &mut impl eval::Input<'stmt>) -> Result<bool> {
        match self.eval_ref(input)? {
            Value::Bool(ret) => Ok(ret),
            _ => todo!(),
        }
    }

    pub(crate) fn eval_ref(&self, input: &mut impl eval::Input<'stmt>) -> Result<Value<'stmt>> {
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
            /*
            Expr::Enum(expr_enum) => Ok(ValueEnum {
                variant: expr_enum.variant,
                fields: expr_enum.fields.eval_ref(input)?,
            }
            .into()),
            */
            Expr::Project(expr_project) => match expr_project.base {
                ProjectBase::ExprSelf => {
                    Ok(input.resolve_self_projection(&expr_project.projection))
                }
                _ => todo!(),
            },
            Expr::Record(expr_record) => Ok(expr_record.eval_ref(input)?.into()),
            Expr::List(exprs) => {
                let mut applied = vec![];

                for expr in &exprs.items {
                    applied.push(expr.eval_ref(input)?);
                }

                Ok(Value::List(applied))
            }
            Expr::Map(expr_map) => {
                let base = expr_map.base.eval_ref(input)?;
                expr_map.map.eval(&base)
            }
        }
    }
}
