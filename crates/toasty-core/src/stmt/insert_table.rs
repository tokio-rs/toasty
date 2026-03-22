use super::InsertTarget;
use crate::schema::db::{ColumnId, TableId};

/// A lowered insert target specifying a database table and its columns.
///
/// Used as the `Table` variant of [`InsertTarget`] after the query engine
/// lowers model-level inserts to table-level operations.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::InsertTable;
/// use toasty_core::schema::db::{TableId, ColumnId};
///
/// let target = InsertTable {
///     table: TableId(0),
///     columns: vec![ColumnId { table: TableId(0), index: 0 }],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct InsertTable {
    /// The database table to insert into.
    pub table: TableId,

    /// The columns to populate, in order matching the value rows.
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
