use crate::{driver, stmt, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Boolean,
    Integer,
    Text,
    VarChar(usize),
}

impl Type {
    pub fn from_app(ty: &stmt::Type) -> Type {
        match ty {
            stmt::Type::Bool => Type::Boolean,
            stmt::Type::I64 => Type::Integer,
            stmt::Type::String => Type::Text,
            _ => todo!("ty={:#?}", ty),
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
