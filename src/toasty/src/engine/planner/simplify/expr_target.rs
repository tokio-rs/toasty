use super::*;

/// The "root" an expression is targetting. This can be a model, table, ...
pub(crate) enum ExprTarget<'a> {
    /// The expression is in context of a model before the expression has been
    /// lowered.
    Model(&'a Model),

    /// The expression has already been lowered and is in context of a table
    Table(&'a Table),
}

impl ExprTarget<'_> {
    pub(crate) fn is_model(&self) -> bool {
        matches!(self, ExprTarget::Model(_))
    }
}

impl<'a> From<&'a Model> for ExprTarget<'a> {
    fn from(value: &'a Model) -> Self {
        ExprTarget::Model(value)
    }
}

impl<'a> From<&'a Table> for ExprTarget<'a> {
    fn from(value: &'a Table) -> Self {
        ExprTarget::Table(value)
    }
}
