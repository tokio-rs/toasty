use crate::engine::plan::{Action, Output, VarId};

#[derive(Debug)]
pub(crate) struct Project {
    /// Source of the input
    pub(crate) input: VarId,

    /// How to project the input, and where to store it.
    pub(crate) output: Output,
}

impl From<Project> for Action {
    fn from(value: Project) -> Self {
        Action::Project(value)
    }
}