use std::fmt;

use super::{Expr, ExprSetOp, Insert, Select, SourceModel, Update, Values};
use crate::schema::db::TableId;

/// A set of rows produced by a query, set operation, or explicit values.
///
/// Represents the different ways to produce a collection of rows in SQL.
///
/// # Examples
///
/// ```text
/// SELECT * FROM users           // ExprSet::Select
/// SELECT ... UNION SELECT ...   // ExprSet::SetOp
/// VALUES (1, 'a'), (2, 'b')     // ExprSet::Values
/// ```
#[derive(Clone, PartialEq)]
pub enum ExprSet {
    /// A select query, possibly with a filter.
    Select(Box<Select>),

    /// A set operation (union, intersection, ...) on two queries.
    SetOp(ExprSetOp),

    /// An update expression.
    Update(Box<Update>),

    /// Explicitly listed values (as expressions).
    Values(Values),

    /// An insert statement (used for UNION-style batch inserts)
    Insert(Box<Insert>),
}

impl ExprSet {
    pub fn values(values: impl Into<Values>) -> ExprSet {
        ExprSet::Values(values.into())
    }

    /// Returns `true` if this is an [`ExprSet::Values`] variant.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::{ExprSet, Values};
    /// let values = ExprSet::values(Values::default());
    /// assert!(values.is_values());
    ///
    /// let select = ExprSet::from(toasty_core::schema::db::TableId(0));
    /// assert!(!select.is_values());
    /// ```
    pub fn is_values(&self) -> bool {
        matches!(self, ExprSet::Values(_))
    }

    /// Returns a reference to the inner [`Values`] if this is an [`ExprSet::Values`].
    ///
    /// Returns `None` for all other [`ExprSet`] variants.
    #[track_caller]
    pub fn as_values(&self) -> Option<&Values> {
        match self {
            Self::Values(values) => Some(values),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`Values`].
    ///
    /// # Panics
    ///
    /// Panics if `self` is not an [`ExprSet::Values`].
    #[track_caller]
    pub fn as_values_unwrap(&self) -> &Values {
        match self {
            Self::Values(values) => values,
            v => panic!("expected `Values`, found {v:#?}"),
        }
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
            ExprSet::Insert(..) => false,
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
            Self::Insert(e) => e.fmt(f),
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

impl From<Insert> for ExprSet {
    fn from(value: Insert) -> Self {
        Self::Insert(Box::new(value))
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
