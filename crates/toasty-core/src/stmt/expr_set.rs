use std::fmt;

use super::{Expr, ExprSetOp, Select, SourceModel, Update, Values};
use crate::schema::db::TableId;

#[derive(Clone, PartialEq)]
pub enum ExprSet {
    /// A select query, possibly with a filter.
    Select(Box<Select>),

    /// A set operation (union, intersection, ...) on two queries
    SetOp(ExprSetOp),

    /// An update expression
    Update(Box<Update>),

    /// Explicitly listed values (as expressions)
    Values(Values),
}

impl ExprSet {
    pub fn values(values: impl Into<Values>) -> ExprSet {
        ExprSet::Values(values.into())
    }

    #[track_caller]
    pub fn as_values_mut(&mut self) -> &mut Values {
        match self {
            Self::Values(expr) => expr,
            _ => todo!(),
        }
    }

    #[track_caller]
    pub fn into_values(self) -> Values {
        match self {
            Self::Values(expr) => expr,
            _ => todo!(),
        }
    }

    pub fn is_const(&self) -> bool {
        match self {
            ExprSet::Select(..) => false,
            ExprSet::SetOp(expr_set_op) => expr_set_op
                .operands
                .iter()
                .all(|operand| operand.is_const()),
            ExprSet::Update(..) => false,
            ExprSet::Values(values) => values.is_const(),
        }
    }
}

impl fmt::Debug for ExprSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Select(e) => e.fmt(f),
            Self::SetOp(e) => e.fmt(f),
            Self::Update(e) => e.fmt(f),
            Self::Values(e) => e.fmt(f),
        }
    }
}

impl Default for ExprSet {
    fn default() -> Self {
        Self::Values(Values::default())
    }
}

impl From<Select> for ExprSet {
    fn from(value: Select) -> Self {
        Self::Select(Box::new(value))
    }
}

impl From<Update> for ExprSet {
    fn from(value: Update) -> Self {
        Self::Update(Box::new(value))
    }
}

impl From<TableId> for ExprSet {
    fn from(value: TableId) -> Self {
        Self::Select(Box::new(Select::from(value)))
    }
}

impl From<SourceModel> for ExprSet {
    fn from(value: SourceModel) -> Self {
        Self::Select(Box::new(Select::from(value)))
    }
}

impl From<Vec<Expr>> for ExprSet {
    fn from(value: Vec<Expr>) -> Self {
        Self::Values(Values::new(value))
    }
}
