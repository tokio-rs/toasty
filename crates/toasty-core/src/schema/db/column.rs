use super::{table, DiffContext, TableId, Type};
use crate::stmt;

use std::{collections::{HashMap, HashSet}, fmt};

#[derive(Debug, PartialEq)]
pub struct Column {
    /// Uniquely identifies the column in the schema.
    pub id: ColumnId,

    /// The name of the column in the database.
    pub name: String,

    /// The column type, from Toasty's point of view.
    pub ty: stmt::Type,

    /// The database storage type of the column.
    pub storage_ty: Type,

    /// Whether or not the column is nullable
    pub nullable: bool,

    /// True if the column is part of the table's primary key
    pub primary_key: bool,

    /// True if the column is an integer that should be auto-incremented
    /// with each insertion of a new row. This should be false if a `storage_ty`
    /// of type `Serial` is used.
    pub auto_increment: bool,
}

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub struct ColumnId {
    pub table: TableId,
    pub index: usize,
}

impl Column {
    fn has_diff(&self, other: &Column) -> bool {
        self.name != other.name
            || self.storage_ty != other.storage_ty
            || self.nullable != other.nullable
            || self.primary_key != other.primary_key
            || self.auto_increment != other.auto_increment
    }
}

impl ColumnId {
    pub(crate) fn placeholder() -> Self {
        Self {
            table: table::TableId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl From<&Column> for ColumnId {
    fn from(value: &Column) -> Self {
        value.id
    }
}

impl fmt::Debug for ColumnId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "ColumnId({}/{})", self.table.0, self.index)
    }
}

pub struct ColumnsDiff<'a> {
    items: Vec<ColumnsDiffItem<'a>>,
}

impl<'a> ColumnsDiff<'a> {
    pub fn from(cx: &DiffContext<'a>, from: &'a [Column], to: &'a [Column]) -> Self {
        let mut items = vec![];
        let mut add_ids: HashSet<_> = to.iter().map(|to| to.id).collect();

        let to_map =
            HashMap::<&str, &'a Column>::from_iter(to.iter().map(|to| (to.name.as_str(), to)));

        for from in from {
            let to = if let Some(to_id) = cx.rename_hints().get_column(from.id) {
                cx.schema_to().column(to_id)
            } else if let Some(to) = to_map.get(from.name.as_str()) {
                to
            } else {
                items.push(ColumnsDiffItem::DropColumn(from));
                continue;
            };

            add_ids.remove(&to.id);

            if from.has_diff(to) {
                items.push(ColumnsDiffItem::AlterColumn { from, to });
            }
        }

        for column_id in add_ids {
            items.push(ColumnsDiffItem::AddColumn(cx.schema_to().column(column_id)));
        }

        Self { items }
    }

    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

pub enum ColumnsDiffItem<'a> {
    AddColumn(&'a Column),
    DropColumn(&'a Column),
    AlterColumn { from: &'a Column, to: &'a Column },
}
