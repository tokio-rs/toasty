use toasty_core::stmt;

use crate::engine::plan::{Output2, VarId};

use super::{Action, Input, Output};

#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Where to get arguments for this action
    pub input: Option<Input>,

    /// How to handle output
    pub output: Option<Output>,

    /// Read statement
    pub read: stmt::Query,

    /// Write statement
    pub write: stmt::Statement,
}

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

impl From<ReadModifyWrite> for Action {
    fn from(value: ReadModifyWrite) -> Self {
        Self::ReadModifyWrite(Box::new(value))
    }
}

impl From<ReadModifyWrite2> for Action {
    fn from(value: ReadModifyWrite2) -> Self {
        Self::ReadModifyWrite2(Box::new(value))
    }
}
