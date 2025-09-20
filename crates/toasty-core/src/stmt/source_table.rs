use super::{TableRef, TableWithJoins};

#[derive(Debug, Clone)]
pub struct SourceTable {
    /// All tables referenced in the statement
    pub tables: Vec<TableRef>,

    /// The main table with joins
    pub from_item: TableWithJoins,
}

impl SourceTable {
    pub fn new(tables: Vec<TableRef>, from_item: TableWithJoins) -> Self {
        Self { tables, from_item }
    }
}
