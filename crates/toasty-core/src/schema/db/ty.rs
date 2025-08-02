use crate::{driver, stmt, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// A boolean value
    Boolean,

    /// A signed integer of `n` bytes
    Integer(u8),

    /// Unconstrained text type
    Text,

    /// Arbitrary precision numeric type (for large unsigned integers)
    Numeric,

    VarChar(u64),
}

impl Type {
    /// Maps an application-level type to a database-level storage type.
    pub fn from_app(
        ty: &stmt::Type,
        hint: &Option<Type>,
        db: &driver::StorageTypes,
    ) -> Result<Type> {
        match hint.clone() {
            Some(ty) => Ok(ty),
            None => match ty {
                stmt::Type::Bool => Ok(Type::Boolean),
                &stmt::Type::I8 => Ok(Type::Integer(1)),
                &stmt::Type::I16 => Ok(Type::Integer(2)),
                &stmt::Type::I32 => Ok(Type::Integer(4)),
                stmt::Type::I64 => Ok(Type::Integer(8)),
                // Map unsigned types to larger signed types to prevent overflow
                &stmt::Type::U8 => Ok(Type::Integer(2)),  // u8 -> SMALLINT (i16)
                &stmt::Type::U16 => Ok(Type::Integer(4)), // u16 -> INTEGER (i32)
                &stmt::Type::U32 => Ok(Type::Integer(8)), // u32 -> BIGINT (i64)
                stmt::Type::U64 => Ok(Type::Numeric),     // u64 -> NUMERIC (arbitrary precision)
                stmt::Type::String => Ok(db.default_string_type.clone()),
                // Gotta support some app-level types as well for now.
                //
                // TODO: not really correct, but we are getting rid of ID types
                // most likely.
                stmt::Type::Id(_) => Ok(db.default_string_type.clone()),
                _ => anyhow::bail!("unsupported type: {ty:?}"),
            },
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        match *self {
            Type::VarChar(size) => match db.storage_types.varchar {
                Some(max) if size > max => {
                    anyhow::bail!("max varchar capacity exceeded: {size} > {max}")
                }
                None => anyhow::bail!("varchar storage type not supported"),
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }
}
