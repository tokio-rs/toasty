use toasty_core::stmt;

use std::fmt;

#[derive(Debug, Clone)]
pub enum ColumnType {
    Boolean,
    Integer,
    Text,
    VarChar(usize),
}

impl ColumnType {
    pub(crate) fn from_schema(ty: &stmt::Type, indexed: bool) -> Self {
        match ty {
            stmt::Type::Bool => Self::Boolean,
            stmt::Type::I8 => Self::Integer,
            stmt::Type::I16 => Self::Integer,
            stmt::Type::I32 => Self::Integer,
            stmt::Type::I64 => Self::Integer,
            stmt::Type::U8 => Self::Integer,
            stmt::Type::U16 => Self::Integer,
            stmt::Type::U32 => Self::Integer,
            stmt::Type::U64 => Self::Integer,
            stmt::Type::String => {
                if indexed {
                    Self::VarChar(255)
                } else {
                    Self::Text
                }
            }
            _ => todo!("ty={:#?}", ty),
        }
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean => write!(f, "BOOLEAN"),
            Self::Integer => write!(f, "INTEGER"),
            Self::Text => write!(f, "TEXT"),
            Self::VarChar(size) => write!(f, "VARCHAR({})", size),
        }
    }
}
