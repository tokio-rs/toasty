use super::Statement;

use toasty_core::schema::db::EnumVariant;

/// An `ALTER TYPE ... ADD VALUE '...'` statement.
///
/// PostgreSQL requires a separate `ALTER TYPE` for each new variant. This
/// statement represents adding a single variant to an existing named enum type.
#[derive(Debug, Clone)]
pub struct AlterType {
    /// The name of the enum type to alter.
    pub type_name: String,
    /// The new variant to add.
    pub variant: EnumVariant,
}

impl Statement {
    /// Creates an `ALTER TYPE <name> ADD VALUE '<variant>'` statement.
    pub fn alter_type_add_value(type_name: &str, variant: &EnumVariant) -> Self {
        AlterType {
            type_name: type_name.to_string(),
            variant: variant.clone(),
        }
        .into()
    }
}

impl From<AlterType> for Statement {
    fn from(value: AlterType) -> Self {
        Self::AlterType(value)
    }
}
