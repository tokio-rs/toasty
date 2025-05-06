use super::*;

#[derive(Debug, Clone)]
pub enum ExprSet {
    /// A select query, possibly with a filter.
    Select(Select),

    /// A set operation (union, intersection, ...) on two queries
    SetOp(ExprSetOp),

    /// An update expression
    Update(Update),

    /// Explicitly listed values (as expressions)
    Values(Values),
}

impl ExprSet {
    #[track_caller]
    pub fn as_select(&self) -> &Select {
        match self {
            Self::Select(expr) => expr,
            _ => todo!("expected Select, but was not; expr_set={:#?}", self),
        }
    }

    #[track_caller]
    pub fn as_select_mut(&mut self) -> &mut Select {
        match self {
            Self::Select(expr) => expr,
            _ => todo!("expected Select, but was not"),
        }
    }

    #[track_caller]
    pub fn into_select(self) -> Select {
        match self {
            Self::Select(expr) => expr,
            _ => todo!(),
        }
    }

    pub fn is_select(&self) -> bool {
        matches!(self, Self::Select(_))
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

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        match self {
            Self::Select(expr) => expr.substitute_ref(input),
            Self::SetOp(expr) => expr.substitute_ref(input),
            Self::Update(_) => todo!(),
            Self::Values(expr) => expr.substitute_ref(input),
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
        Self::Select(value)
    }
}
