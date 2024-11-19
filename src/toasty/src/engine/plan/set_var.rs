use super::*;

#[derive(Debug)]
pub(crate) struct SetVar {
    pub var: VarId,
    pub value: Vec<stmt::Value<'static>>,
}

impl<'stmt> Action<'stmt> {
    pub(crate) fn into_set_var(self) -> SetVar {
        match self {
            Action::SetVar(action) => action,
            _ => panic!(),
        }
    }
}

impl<'stmt> From<SetVar> for Action<'stmt> {
    fn from(src: SetVar) -> Action<'stmt> {
        Action::SetVar(src)
    }
}
