use super::*;

use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Column {
    /// Uniquely identifies the column in the schema.
    pub id: ColumnId,

    /// The name of the column
    pub name: String,

    /// The column type, from Toasty's point of view.
    pub ty: stmt::Type,

    /// The database storae type of the column.
    ///
    /// When `None`, no specific storage type has been requested.
    pub storage_ty: Option<Type>,

    /// Whether or not the column is nullable
    pub nullable: bool,

    /// True if the column is part of the table's primary key
    pub primary_key: bool,
}

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub struct ColumnId {
    pub table: TableId,
    pub index: usize,
}

impl Column {}

impl ColumnId {
    pub(crate) fn placeholder() -> Self {
        Self {
            table: table::TableId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl fmt::Debug for ColumnId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "ColumnId({}/{})", self.table.0, self.index)
    }
}
