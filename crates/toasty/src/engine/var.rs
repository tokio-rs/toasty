use crate::schema::stmt::{Path, Record, ValueCow};

#[derive(Debug, Clone)]
pub(crate) enum Var<'a> {
    Value(Option<ValueCow<'a>>),
    Path(Path),
}

impl<'a> Var<'a> {
    pub(crate) fn value(value: impl Into<ValueCow<'a>>) -> Var<'a> {
        Var::Value(Some(value.into()))
    }

    pub(crate) fn path(path: impl Into<Path>) -> Var<'a> {
        Var::Path(path.into())
    }

    pub(crate) fn apply(&self, record: &mut Record<'a>) -> Option<ValueCow<'a>> {
        match self {
            Self::Value(Some(v)) => Some(v.clone()),
            Self::Value(None) => None,
            Self::Path(path) => {
                let [field] = path.steps() else {
                    panic!("invalid path cardinality; self={:?}", self)
                };
                record.take(field.as_index())
            }
        }
    }
}
