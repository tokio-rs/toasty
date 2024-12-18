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

#[derive(Debug, PartialEq)]
pub struct IndexColumn {
    /// The column being indexed
    pub column: ColumnId,

    /// The comparison operation used to index the column
    pub op: IndexOp,

    /// Scope of the index
    pub scope: IndexScope,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum IndexOp {
    Eq,
    Sort(stmt::Direction),
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum IndexScope {
    /// The index column is used to partition rows across nodes of a distributed database.
    Partition,

    /// The index column is scoped to a physical node.
    Local,
}

impl Index {
    pub fn key_ty(&self, schema: &Schema) -> stmt::Type {
        match &self.columns[..] {
            [id] => schema.column(id).ty.clone(),
            ids => stmt::Type::Record(ids.iter().map(|id| schema.column(id).ty.clone()).collect()),
        }
    }
}

impl IndexColumn {
    pub fn table_column<'a>(&self, schema: &'a Schema) -> &'a Column {
        schema.column(self.column)
    }
}

impl Into<ColumnId> for &IndexColumn {
    fn into(self) -> ColumnId {
        self.column
    }
}

impl IndexScope {
    pub fn is_partition(self) -> bool {
        matches!(self, IndexScope::Partition)
    }

    pub fn is_local(self) -> bool {
        matches!(self, IndexScope::Local)
    }
}

impl IndexId {
    pub(crate) fn placeholder() -> IndexId {
        IndexId {
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
