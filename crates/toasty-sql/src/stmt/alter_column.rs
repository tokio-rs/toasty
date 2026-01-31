use crate::stmt::ColumnDef;

use super::Statement;

use toasty_core::{
    driver::Capability,
    schema::db::{Column, TableId, Type},
};

/// A statement to alter a column in a table.
#[derive(Debug, Clone)]
pub struct AlterColumn {
    /// ID of the table containing the column.
    pub table: TableId,

    /// Current column definition.
    pub column_def: ColumnDef,

    /// New name for the column (if renaming).
    pub new_name: Option<String>,

    /// New type information.
    pub new_ty: Option<Type>,

    /// New nullability constraint.
    pub new_not_null: Option<bool>,

    /// New auto increment behavior.
    pub new_auto_increment: Option<bool>,
}

impl Statement {
    /// Alters a column, detecting what changed between from and to.
    pub fn alter_column(from: &Column, to: &Column, capability: &Capability) -> Self {}
}

impl From<AlterColumn> for Statement {
    fn from(value: AlterColumn) -> Self {
        Self::AlterColumn(value)
    }
}
