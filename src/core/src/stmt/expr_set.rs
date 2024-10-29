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
