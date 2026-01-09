use super::{Column, ColumnId, Index, IndexId, PrimaryKey};
use crate::{
    schema::db::{column::ColumnsDiff, diff::DiffContext, index::IndicesDiff},
    stmt,
};

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

/// A database table
#[derive(Debug)]
pub struct Table {
    /// Uniquely identifies a table
    pub id: TableId,

    /// Name of the table
    pub name: String,

    /// The table's columns
    pub columns: Vec<Column>,

    pub primary_key: PrimaryKey,

    pub indices: Vec<Index>,
}

/// Uniquely identifies a table
#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub struct TableId(pub usize);

impl Table {
    pub fn primary_key_column(&self, i: usize) -> &Column {
        &self.columns[self.primary_key.columns[i].index]
    }

    pub fn primary_key_columns(&self) -> impl ExactSizeIterator<Item = &Column> + '_ {
        self.primary_key
            .columns
            .iter()
            .map(|column_id| &self.columns[column_id.index])
    }

    pub fn column(&self, id: impl Into<ColumnId>) -> &Column {
        &self.columns[id.into().index]
    }

    /// The path must have exactly one step
    pub fn resolve(&self, projection: &stmt::Projection) -> &Column {
        let [first, rest @ ..] = projection.as_slice() else {
            panic!("need at most one path step")
        };
        assert!(rest.is_empty());

        &self.columns[*first]
    }

    pub(crate) fn new(id: TableId, name: String) -> Self {
        Self {
            id,
            name,
            columns: vec![],
            primary_key: PrimaryKey {
                columns: vec![],
                index: IndexId {
                    table: id,
                    index: 0,
                },
            },
            indices: vec![],
        }
    }
}

impl TableId {
    pub(crate) fn placeholder() -> Self {
        Self(usize::MAX)
    }
}

impl fmt::Debug for TableId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "TableId({})", self.0)
    }
}

pub struct TablesDiff<'a> {
    items: Vec<TablesDiffItem<'a>>,
}

impl<'a> TablesDiff<'a> {
    pub fn from(cx: &DiffContext<'a>, from: &'a [Table], to: &'a [Table]) -> Self {
        let mut items = vec![];
        let mut create_ids: HashSet<_> = to.iter().map(|to| to.id).collect();

        let to_map =
            HashMap::<&str, &'a Table>::from_iter(to.iter().map(|to| (to.name.as_str(), to)));

        for from in from {
            let to = if let Some(to_id) = cx.rename_hints().get_table(from.id) {
                cx.schema_to().table(to_id)
            } else if let Some(to) = to_map.get(from.name.as_str()) {
                to
            } else {
                items.push(TablesDiffItem::DropTable(from));
                continue;
            };

            create_ids.remove(&to.id);

            let columns = ColumnsDiff::from(&from.columns, &to.columns);
            let indices = IndicesDiff::from(&from.indices, &to.indices);
            if from.name != to.name || !columns.is_empty() || !indices.is_empty() {
                items.push(TablesDiffItem::AlterTable {
                    from,
                    to,
                    columns,
                    indices,
                });
            }
        }

        for table_id in create_ids {
            items.push(TablesDiffItem::CreateTable(cx.schema_to().table(table_id)));
        }

        Self { items }
    }
}

pub enum TablesDiffItem<'a> {
    CreateTable(&'a Table),
    DropTable(&'a Table),
    AlterTable {
        from: &'a Table,
        to: &'a Table,
        columns: ColumnsDiff<'a>,
        indices: IndicesDiff<'a>,
    },
}
