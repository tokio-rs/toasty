use super::{Column, ColumnId, DiffContext, Schema, TableId};
use crate::stmt;

use std::{collections::{HashMap, HashSet}, fmt};

#[derive(Debug)]
pub struct Index {
    /// Uniquely identifies the index within the schema
    pub id: IndexId,

    /// Index name is unique within the schema
    pub name: String,

    /// The table being indexed
    pub on: TableId,

    /// Fields included in the index.
    pub columns: Vec<IndexColumn>,

    /// When `true`, indexed entries are unique
    pub unique: bool,

    /// When `true`, the index indexes the model's primary key fields.
    pub primary_key: bool,
}

impl Index {
    fn has_diff(&self, other: &Index) -> bool {
        self.name != other.name
            || self.columns.len() != other.columns.len()
            || self
                .columns
                .iter()
                .zip(other.columns.iter())
                .any(|(s, o)| s.op != o.op || s.scope != o.scope)
            || self.unique != other.unique
            || self.primary_key != other.primary_key
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct IndexId {
    pub table: TableId,
    pub index: usize,
}

#[derive(Debug)]
pub struct IndexColumn {
    /// The column being indexed
    pub column: ColumnId,

    /// The comparison operation used to index the column
    pub op: IndexOp,

    /// Scope of the index
    pub scope: IndexScope,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IndexOp {
    Eq,
    Sort(stmt::Direction),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IndexScope {
    /// The index column is used to partition rows across nodes of a distributed database.
    Partition,

    /// The index column is scoped to a physical node.
    Local,
}

impl IndexColumn {
    pub fn table_column<'a>(&self, schema: &'a Schema) -> &'a Column {
        schema.column(self.column)
    }
}

impl IndexScope {
    pub fn is_partition(self) -> bool {
        matches!(self, Self::Partition)
    }

    pub fn is_local(self) -> bool {
        matches!(self, Self::Local)
    }
}

impl IndexId {
    pub(crate) fn placeholder() -> Self {
        Self {
            table: TableId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl fmt::Debug for IndexId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "IndexId({}/{})", self.table.0, self.index)
    }
}

pub struct IndicesDiff<'a> {
    items: Vec<IndicesDiffItem<'a>>,
}

impl<'a> IndicesDiff<'a> {
    pub fn from(cx: &DiffContext<'a>, from: &'a [Index], to: &'a [Index]) -> Self {
        let mut items = vec![];
        let mut create_ids: HashSet<_> = to.iter().map(|to| to.id).collect();

        let to_map =
            HashMap::<&str, &'a Index>::from_iter(to.iter().map(|to| (to.name.as_str(), to)));

        for from in from {
            let to = if let Some(to_id) = cx.rename_hints().get_index(from.id) {
                cx.schema_to().index(to_id)
            } else if let Some(to) = to_map.get(from.name.as_str()) {
                to
            } else {
                items.push(IndicesDiffItem::DropIndex(from));
                continue;
            };

            create_ids.remove(&to.id);

            if from.has_diff(to) {
                items.push(IndicesDiffItem::AlterIndex { from, to });
            }
        }

        for index_id in create_ids {
            items.push(IndicesDiffItem::CreateIndex(cx.schema_to().index(index_id)));
        }

        Self { items }
    }

    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

pub enum IndicesDiffItem<'a> {
    CreateIndex(&'a Index),
    DropIndex(&'a Index),
    AlterIndex { from: &'a Index, to: &'a Index },
}
