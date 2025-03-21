use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct InsertTable {
    /// Table identifier to insert into
    pub table: TableId,

    /// Columns to insert into
    pub columns: Vec<ColumnId>,
}

impl From<InsertTable> for InsertTarget {
    fn from(value: InsertTable) -> Self {
        InsertTarget::Table(value)
    }
}

impl From<&InsertTable> for TableId {
    fn from(value: &InsertTable) -> Self {
        value.table
    }
}
