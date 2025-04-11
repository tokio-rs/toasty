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
    pub(crate) fn from_schema(ty: &stmt::Type, indexed: bool) -> ColumnType {
        match ty {
            stmt::Type::Bool => ColumnType::Boolean,
            stmt::Type::Id(_) => ColumnType::Text,
            stmt::Type::I64 => ColumnType::Integer,
            stmt::Type::String => {
                if indexed {
                    ColumnType::VarChar(255)
                } else {
                    ColumnType::Text
                }
            }
            stmt::Type::Enum(_) => ColumnType::Text,
            _ => todo!("ty={:#?}", ty),
        }
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnType::Boolean => write!(f, "BOOLEAN"),
            ColumnType::Integer => write!(f, "INTEGER"),
            ColumnType::Text => write!(f, "TEXT"),
            ColumnType::VarChar(size) => write!(f, "VARCHAR({})", size),
        }
    }
}
