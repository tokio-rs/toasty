use toasty_core::stmt;

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

impl From<ReadModifyWrite> for Action {
    fn from(value: ReadModifyWrite) -> Self {
        Self::ReadModifyWrite(Box::new(value))
    }
}
