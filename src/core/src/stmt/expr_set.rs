use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ExprSet<'stmt> {
    /// A select query, possibly with a filter.
    Select(Select<'stmt>),

    /// A set operation (union, intersection, ...) on two queries
    SetOp(ExprSetOp<'stmt>),

    /// Explicitly listed values (as expressions)
    Values(Values<'stmt>),
}

impl<'stmt> ExprSet<'stmt> {
    #[track_caller]
    pub fn as_select(&self) -> &Select<'stmt> {
        match self {
            ExprSet::Select(expr) => expr,
            _ => todo!("expected Select, but was not; expr_set={:#?}", self),
        }
    }

    #[track_caller]
    pub fn as_select_mut(&mut self) -> &mut Select<'stmt> {
        match self {
            ExprSet::Select(expr) => expr,
            _ => todo!("expected Select, but was not"),
        }
    }

    #[track_caller]
    pub fn into_select(self) -> Select<'stmt> {
        match self {
            ExprSet::Select(expr) => expr,
            _ => todo!(),
        }
    }

    pub(crate) fn width(&self, schema: &Schema) -> usize {
        match self {
            ExprSet::Select(select) => schema.model(select.source.as_model_id()).fields.len(),
            ExprSet::SetOp(expr_set_op) if expr_set_op.operands.len() == 0 => 0,
            ExprSet::SetOp(expr_set_op) => expr_set_op.operands[0].width(schema),
            ExprSet::Values(values) if values.rows.len() == 0 => 0,
            ExprSet::Values(values) => values.rows[0].len(),
        }
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
        match self {
            ExprSet::Select(expr) => expr.substitute_ref(input),
            ExprSet::SetOp(expr) => expr.substitute_ref(input),
            ExprSet::Values(expr) => expr.substitute_ref(input),
        }
    }
}

impl<'stmt> Default for ExprSet<'stmt> {
    fn default() -> Self {
        ExprSet::Values(Values::default())
    }
}
