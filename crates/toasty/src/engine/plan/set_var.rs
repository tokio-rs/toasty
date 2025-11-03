use toasty_core::driver::Rows;

use crate::engine::plan::Output2;

use super::{stmt, Action};

#[derive(Debug)]
pub(crate) struct SetVar {
    pub var: VarId,
    pub value: Vec<stmt::Value>,
}

#[derive(Debug)]
pub(crate) struct SetVar2 {
    pub rows: Vec<stmt::Value>,
    pub output: Output2,
}

/// Identifies a pipeline variable slot
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct VarId(pub(crate) usize);

impl Action {
    pub(crate) fn into_set_var(self) -> SetVar {
        match self {
            Self::SetVar(action) => action,
            _ => panic!(),
        }
    }
}

impl From<SetVar> for Action {
    fn from(src: SetVar) -> Self {
        Self::SetVar(src)
    }
}

impl From<SetVar2> for Action {
    fn from(value: SetVar2) -> Self {
        Self::SetVar2(value)
    }
}
