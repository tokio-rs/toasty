use crate::stmt::{Query, Source, SourceTableId, TableFactor, Values};

use super::{TableDerived, TableRef, TableWithJoins};

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

impl From<Values> for SourceTable {
    fn from(value: Values) -> Self {
        SourceTable {
            tables: vec![TableRef::Derived(TableDerived {
                subquery: Box::new(Query::new(value)),
            })],
            from_item: TableWithJoins {
                relation: TableFactor::Table(SourceTableId(0)),
                joins: vec![],
            },
        }
    }
}

impl From<SourceTable> for Source {
    fn from(value: SourceTable) -> Self {
        Source::Table(value)
    }
}
