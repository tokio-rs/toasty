use crate::engine::{
    eval,
    plan::{Action, VarId},
};

#[derive(Debug)]
pub(crate) struct Project {
    /// Source of the input
    pub(crate) input: VarId,

    /// Where to store the output
    pub(crate) output: VarId,

    /// How to project it before storing
    pub(crate) projection: eval::Func,
}

impl From<Project> for Action {
    fn from(value: Project) -> Self {
        Action::Project(value)
    }
}
