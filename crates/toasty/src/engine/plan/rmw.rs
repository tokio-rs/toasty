use crate::engine::plan::{Action, Output2, VarId};
use toasty_core::stmt;

#[derive(Debug)]
pub(crate) struct ReadModifyWrite2 {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: Option<Output2>,

    /// Read statement
    pub read: stmt::Query,

    /// Write statement
    pub write: stmt::Statement,
}

impl From<ReadModifyWrite2> for Action {
    fn from(value: ReadModifyWrite2) -> Self {
        Self::ReadModifyWrite2(Box::new(value))
    }
}
