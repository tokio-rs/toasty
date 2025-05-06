use super::*;

#[derive(Debug, Clone)]
pub struct InsertTable {
    /// Table identifier to insert into
    pub table: TableId,

    /// Columns to insert into
    pub columns: Vec<ColumnId>,
}

impl From<InsertTable> for InsertTarget {
    fn from(value: InsertTable) -> Self {
        Self::Table(value)
    }
}

impl From<&InsertTable> for TableId {
    fn from(value: &InsertTable) -> Self {
        value.table
    }
}
