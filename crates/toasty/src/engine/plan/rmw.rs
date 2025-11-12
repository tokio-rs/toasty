use crate::engine::plan::{Action, Output, VarId};
use toasty_core::stmt;

#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: Option<Output>,

    /// Read statement
    pub read: stmt::Query,

    /// Write statement
    pub write: stmt::Statement,
}

impl From<ReadModifyWrite> for Action {
    fn from(value: ReadModifyWrite) -> Self {
        Self::ReadModifyWrite(Box::new(value))
    }
}
