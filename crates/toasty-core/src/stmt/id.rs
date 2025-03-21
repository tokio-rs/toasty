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

impl Value {
    pub const fn is_id(&self) -> bool {
        matches!(self, Value::Id(_))
    }

    pub fn into_id(self) -> Id {
        match self {
            Value::Id(id) => id,
            _ => todo!(),
        }
    }
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

    pub fn to_primitive(&self) -> stmt::Value {
        match &self.repr {
            Repr::Int(_) => todo!(),
            Repr::String(id) => id.clone().into(),
        }
    }

    pub fn into_primitive(self) -> stmt::Value {
        match self.repr {
            Repr::Int(_) => todo!(),
            Repr::String(id) => id.into(),
        }
    }

    pub fn cast(self, ty: &Type) -> Result<Value> {
        match (self.repr, ty) {
            (Repr::String(id), Type::String) => Ok(id.into()),
            (repr, _) => todo!("id={repr:#?}; ty={ty:#?}"),
        }
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Int(id) => write!(f, "{id}"),
            Repr::String(id) => write!(f, "{id}"),
        }
    }
}

impl From<Id> for Expr {
    fn from(value: Id) -> Self {
        Expr::Value(value.into())
    }
}

impl From<&Id> for Expr {
    fn from(value: &Id) -> Expr {
        Expr::Value(value.into())
    }
}

impl From<&Id> for stmt::Value {
    fn from(src: &Id) -> stmt::Value {
        // TODO: probably can avoid cloning if needed
        stmt::Value::Id(src.to_owned())
    }
}

impl From<Id> for stmt::Value {
    fn from(src: Id) -> stmt::Value {
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
