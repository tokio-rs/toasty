use crate::engine::plan::{Action, Output};
use toasty_core::stmt;

#[derive(Debug)]
pub(crate) struct SetVar {
    pub rows: Vec<stmt::Value>,
    pub output: Output,
}

/// Identifies a pipeline variable slot
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct VarId(pub(crate) usize);

impl From<SetVar> for Action {
    fn from(value: SetVar) -> Self {
        Self::SetVar(value)
    }
}
