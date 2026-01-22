use super::{Expr, Type, Value};
use crate::{schema::app::ModelId, stmt, Result};
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
        matches!(self, Self::Id(_))
    }

    pub fn into_id(self) -> Id {
        match self {
            Self::Id(id) => id,
            _ => todo!(),
        }
    }
}

impl Id {
    pub fn from_int(model: ModelId, id: u64) -> Self {
        Self {
            model,
            repr: Repr::Int(id),
        }
    }

    pub fn from_string(model: ModelId, string: String) -> Self {
        Self {
            model,
            repr: Repr::String(string),
        }
    }

    /// The model this identifier represents
    pub fn model_id(&self) -> ModelId {
        self.model
    }

    /// Return an integer representation of the record identifier.
    pub fn to_int(&self) -> Result<u64> {
        match &self.repr {
            Repr::Int(id) => Ok(*id),
            Repr::String(_) => Err(crate::err!("Id not an int")),
        }
    }

    /// Return a string representation of the record identifier.
    pub fn as_str(&self) -> Result<&str> {
        match &self.repr {
            Repr::String(id) => Ok(id.as_str()),
            Repr::Int(_) => Err(crate::err!("Id not a string")),
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
            (repr, Type::Id(model_id)) if *model_id == self.model => Ok(Id {
                model: self.model,
                repr,
            }
            .into()),
            (Repr::String(id), Type::String) => Ok(id.into()),
            (repr, _) => todo!(
                "id={:#?}; ty={ty:#?}",
                Id {
                    model: self.model,
                    repr
                }
            ),
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
        Self::Value(value.into())
    }
}

impl From<&Id> for Expr {
    fn from(value: &Id) -> Self {
        Self::Value(value.into())
    }
}

impl From<&Id> for stmt::Value {
    fn from(src: &Id) -> Self {
        // TODO: probably can avoid cloning if needed
        Self::Id(src.to_owned())
    }
}

impl From<Id> for stmt::Value {
    fn from(src: Id) -> Self {
        Self::Id(src)
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
