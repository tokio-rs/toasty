use crate::engine::{
    eval,
    plan::{Action, Output2, VarId},
};

#[derive(Debug)]
pub(crate) struct Filter {
    /// Source of the input
    pub(crate) input: VarId,

    /// Where to store the output
    pub(crate) output: Output2,

    /// How to project it before storing
    pub(crate) filter: eval::Func,
}

impl From<Filter> for Action {
    fn from(value: Filter) -> Self {
        Action::Filter(value)
    }
}
