use crate::engine::plan::{Action, Output2};
use toasty_core::stmt;

#[derive(Debug)]
pub(crate) struct SetVar2 {
    pub rows: Vec<stmt::Value>,
    pub output: Output2,
}

/// Identifies a pipeline variable slot
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct VarId(pub(crate) usize);

impl From<SetVar2> for Action {
    fn from(value: SetVar2) -> Self {
        Self::SetVar2(value)
    }
}
