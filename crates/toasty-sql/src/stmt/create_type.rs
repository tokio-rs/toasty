use super::Statement;

use toasty_core::schema::db::TypeEnum;

/// A `CREATE TYPE ... AS ENUM (...)` statement.
#[derive(Debug, Clone)]
pub struct CreateType {
    /// The enum type definition.
    pub ty: TypeEnum,
}

impl Statement {
    /// Creates a `CREATE TYPE ... AS ENUM (...)` statement from a [`TypeEnum`].
    pub fn create_enum_type(ty: &TypeEnum) -> Self {
        CreateType { ty: ty.clone() }.into()
    }
}

impl From<CreateType> for Statement {
    fn from(value: CreateType) -> Self {
        Self::CreateType(value)
    }
}
