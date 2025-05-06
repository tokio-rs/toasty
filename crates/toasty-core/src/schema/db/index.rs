use super::*;

use std::fmt;

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

#[derive(Copy, Clone, Eq, PartialEq)]
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

#[derive(Debug, Copy, Clone)]
pub enum IndexOp {
    Eq,
    Sort(stmt::Direction),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
