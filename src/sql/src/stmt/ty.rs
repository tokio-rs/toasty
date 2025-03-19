use toasty_core::stmt;

use std::fmt;

#[derive(Debug, Clone)]
pub enum Type {
    Boolean,
    Integer,
    Text,
}

impl Type {
    pub(crate) fn from_schema(ty: &stmt::Type) -> Type {
        match ty {
            stmt::Type::Bool => Type::Boolean,
            stmt::Type::Id(_) => Type::Text,
            stmt::Type::I64 => Type::Integer,
            stmt::Type::String => Type::Text,
            stmt::Type::Enum(_) => Type::Text,
            _ => todo!("ty={:#?}", ty),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Boolean => write!(f, "BOOLEAN"),
            Type::Integer => write!(f, "INTEGER"),
            Type::Text => write!(f, "TEXT"),
        }
    }
}
