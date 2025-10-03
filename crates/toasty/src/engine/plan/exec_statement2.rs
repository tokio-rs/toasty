use crate::engine::plan::VarId;

use super::{stmt, Action, Input, Output};

#[derive(Debug)]
pub(crate) struct ExecStatement2 {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: Option<VarId>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,

    /// HAX: this should be handled more generically, but for now, lets just get
    /// it working.
    pub conditional_update_with_no_returning: bool,
}

impl From<ExecStatement2> for Action {
    fn from(value: ExecStatement2) -> Self {
        Self::ExecStatement2(value)
    }
}
