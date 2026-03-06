use crate::stmt::ColumnDef;

use super::Statement;

use toasty_core::{
    driver::Capability,
    schema::db::{Column, ColumnId, Type},
};

/// A statement to alter a column in a table.
#[derive(Debug, Clone)]
pub struct AlterColumn {
    /// ID of the column being altered.
    pub id: ColumnId,

    /// Current column definition.
    pub column_def: ColumnDef,

    /// Changes to be made to the column.
    pub changes: AlterColumnChanges,
}

/// A statement to alter a column in a table.
#[derive(Debug, Clone)]
pub struct AlterColumnChanges {
    /// New name for the column (if renaming).
    pub new_name: Option<String>,

    /// New type information.
    pub new_ty: Option<Type>,

    /// New nullability constraint.
    pub new_not_null: Option<bool>,

    /// New auto increment behavior.
    pub new_auto_increment: Option<bool>,
}

impl AlterColumnChanges {
    pub fn from_diff(previous: &Column, next: &Column) -> Self {
        Self {
            new_name: (previous.name != next.name).then(|| next.name.clone()),
            new_ty: (previous.storage_ty != next.storage_ty).then(|| next.storage_ty.clone()),
            new_not_null: (previous.nullable != next.nullable).then_some(!next.nullable),
            new_auto_increment: (previous.auto_increment != next.auto_increment)
                .then_some(next.auto_increment),
        }
    }

    /// Splits up this set of changes into a [`Vec`] of individual changes.
    pub fn split(self) -> Vec<Self> {
        let Self {
            new_name,
            new_ty,
            new_not_null,
            new_auto_increment,
        } = self;
        let default = AlterColumnChanges {
            new_name: None,
            new_ty: None,
            new_not_null: None,
            new_auto_increment: None,
        };
        let mut result = vec![];
        if new_name.is_some() {
            result.push(Self {
                new_name,
                ..default.clone()
            });
        }
        if new_ty.is_some() {
            result.push(Self {
                new_ty,
                ..default.clone()
            });
        }
        if new_not_null.is_some() {
            result.push(Self {
                new_not_null,
                ..default.clone()
            });
        }
        if new_auto_increment.is_some() {
            result.push(Self {
                new_auto_increment,
                ..default.clone()
            });
        }
        result
    }

    pub fn has_type_change(&self) -> bool {
        self.new_ty.is_some() || self.new_not_null.is_some() || self.new_auto_increment.is_some()
    }
}

impl Statement {
    /// Alters a column.
    pub fn alter_column(
        column: &Column,
        changes: AlterColumnChanges,
        capability: &Capability,
    ) -> Self {
        AlterColumn {
            id: column.id,
            column_def: ColumnDef::from_schema(column, &capability.storage_types),
            changes,
        }
        .into()
    }
}

impl From<AlterColumn> for Statement {
    fn from(value: AlterColumn) -> Self {
        Self::AlterColumn(value)
    }
}
