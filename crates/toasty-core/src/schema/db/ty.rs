use crate::{driver, stmt, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// A boolean value
    Boolean,

    /// A signed integer of `n` bytes
    Integer(u8),

    /// An unsigned integer of `n` bytes
    UnsignedInteger(u8),

    /// Unconstrained text type
    Text,

    /// Text type with an explicit maximum length
    VarChar(u64),

    /// 128-bit universally unique identifier (UUID)
    Uuid,

    /// Unconstrained binary type
    Blob,

    /// Fixed-size binary type of `n` bytes
    Binary(u8),

    /// User-specified unrecognized type
    Custom(String),
}

impl Type {
    /// Maps an application-level type to a database-level storage type.
    pub fn from_app(
        ty: &stmt::Type,
        hint: Option<&Type>,
        db: &driver::StorageTypes,
    ) -> Result<Type> {
        match hint {
            Some(ty) => Ok(ty.clone()),
            None => match ty {
                stmt::Type::Bool => Ok(Type::Boolean),
                stmt::Type::I8 => Ok(Type::Integer(1)),
                stmt::Type::I16 => Ok(Type::Integer(2)),
                stmt::Type::I32 => Ok(Type::Integer(4)),
                stmt::Type::I64 => Ok(Type::Integer(8)),
                // Map unsigned types to UnsignedInteger with appropriate byte width
                stmt::Type::U8 => Ok(Type::UnsignedInteger(1)),
                stmt::Type::U16 => Ok(Type::UnsignedInteger(2)),
                stmt::Type::U32 => Ok(Type::UnsignedInteger(4)),
                stmt::Type::U64 => Ok(Type::UnsignedInteger(8)),
                stmt::Type::String => Ok(db.default_string_type.clone()),
                stmt::Type::Uuid => Ok(db.default_uuid_type.clone()),
                // Gotta support some app-level types as well for now.
                //
                // TODO: not really correct, but we are getting rid of ID types
                // most likely.
                stmt::Type::Id(_) => Ok(db.default_string_type.clone()),
                // Enum types are stored as strings in the database
                stmt::Type::Enum(_) => Ok(db.default_string_type.clone()),
                _ => anyhow::bail!("unsupported type: {ty:?}"),
            },
        }
    }

    /// Determines the [`stmt::Type`] closest to this [`db::Type`] that should be used
    /// as an intermediate conversion step to lessen the work done by each individual driver.
    pub fn bridge_type(&self, ty: &stmt::Type) -> stmt::Type {
        match (self, ty) {
            (Self::Blob | Self::Binary(_), stmt::Type::Uuid) => {
                stmt::Type::List(stmt::Type::U8.into())
            }
            (Self::Text | Self::VarChar(_), stmt::Type::Uuid) => stmt::Type::String,
            (Self::Text | Self::VarChar(_), stmt::Type::Id(_)) => stmt::Type::String,
            _ => ty.clone(),
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
