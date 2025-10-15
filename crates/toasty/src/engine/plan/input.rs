use super::{eval, stmt, VarId};

#[derive(Debug, Clone)]
pub(crate) struct Input {
    /// Source of the input
    pub(crate) source: InputSource,

    /// How to project the input
    pub(crate) project: eval::Func,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum InputSource {
    /// Take an entry from the variable table by value. This consumes the entry.
    Value(VarId),
}

impl Input {
    pub(crate) fn from_var(var: VarId, ty: stmt::Type) -> Self {
        Self {
            source: InputSource::Value(var),
            project: eval::Func::identity(ty),
        }
    }
}

impl From<&InputSource> for VarId {
    fn from(value: &InputSource) -> Self {
        match *value {
            InputSource::Value(var_id) => var_id,
        }
    }
}
