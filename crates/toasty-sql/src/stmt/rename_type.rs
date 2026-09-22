use super::Statement;

/// An `ALTER TYPE ... RENAME TO ...` statement.
#[derive(Debug, Clone)]
pub struct RenameType {
    /// The current type name.
    pub type_name: String,
    /// The new type name.
    pub new_name: String,
}

impl Statement {
    /// Creates an `ALTER TYPE <name> RENAME TO <new_name>` statement.
    pub fn rename_type(type_name: &str, new_name: &str) -> Self {
        RenameType {
            type_name: type_name.to_string(),
            new_name: new_name.to_string(),
        }
        .into()
    }
}

impl From<RenameType> for Statement {
    fn from(value: RenameType) -> Self {
        Self::RenameType(value)
    }
}
