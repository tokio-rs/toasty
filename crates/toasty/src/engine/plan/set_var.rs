use super::*;

#[derive(Debug)]
pub(crate) struct SetVar {
    pub var: VarId,
    pub value: Vec<stmt::Value>,
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
