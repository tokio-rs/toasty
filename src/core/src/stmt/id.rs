use super::*;

use std::fmt;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Id {
    /// The model the identifier is associated with.
    model: ModelId,

    /// How the identifier is represented
    repr: Repr,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum Repr {
    Int(u64),
    String(String),
}

impl Id {
    pub fn from_int(model: ModelId, id: u64) -> Id {
        Id {
            model,
            repr: Repr::Int(id),
        }
    }

    pub fn from_string(model: ModelId, string: String) -> Id {
        Id {
            model,
            repr: Repr::String(string),
        }
    }

    /// The model this identifier represents
    pub fn model_id(&self) -> ModelId {
        self.model
    }

    /// Return an integer representation of the record identifier.
    pub fn to_int(&self) -> Result<u64, Error> {
        match &self.repr {
            Repr::Int(id) => Ok(*id),
            Repr::String(_) => anyhow::bail!("Id not an int"),
        }
    }

    /// Return a string representation of the record identifier.
    pub fn to_string(&self) -> String {
        match &self.repr {
            Repr::Int(id) => id.to_string(),
            Repr::String(id) => id.clone(),
        }
    }

    pub fn to_primitive(&self) -> stmt::Value<'_> {
        match &self.repr {
            Repr::Int(_) => todo!(),
            Repr::String(id) => id.into(),
        }
    }
}

impl<'stmt> From<Id> for Expr<'stmt> {
    fn from(value: Id) -> Self {
        Expr::Value(value.into())
    }
}

impl<'stmt> From<&'stmt Id> for Expr<'stmt> {
    fn from(value: &'stmt Id) -> Expr<'stmt> {
        Expr::Value(value.into())
    }
}

impl<'a> From<&'a Id> for stmt::Value<'a> {
    fn from(src: &'a Id) -> stmt::Value<'a> {
        // TODO: probably can avoid cloning if needed
        stmt::Value::Id(src.to_owned())
    }
}

impl<'a> From<Id> for stmt::Value<'a> {
    fn from(src: Id) -> stmt::Value<'a> {
        stmt::Value::Id(src)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Int(id) => fmt.debug_tuple("Id").field(id).finish(),
            Repr::String(id) => fmt.debug_tuple("Id").field(id).finish(),
        }
    }
}
