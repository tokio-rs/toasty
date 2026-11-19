use super::{ColumnDef, Name, Statement};

use toasty_core::{driver::Capability, schema::db::Column};

/// Sets or removes the native comment on a column.
#[derive(Debug, Clone)]
pub struct SetColumnComment {
    /// Database table name.
    pub table: Name,

    /// Complete column definition, including the effective comment.
    pub column: ColumnDef,
}

impl Statement {
    /// Sets or removes a column comment.
    pub fn set_column_comment(
        table: impl Into<Name>,
        column: &Column,
        capability: &Capability,
    ) -> Self {
        let mut column_def = ColumnDef::from_schema(column, &capability.storage_types, capability);
        column_def.comment = column.comment.clone();

        SetColumnComment {
            table: table.into(),
            column: column_def,
        }
        .into()
    }
}

impl From<SetColumnComment> for Statement {
    fn from(value: SetColumnComment) -> Self {
        Self::SetColumnComment(value)
    }
}
