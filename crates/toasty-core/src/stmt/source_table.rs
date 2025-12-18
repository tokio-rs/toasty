use crate::stmt::{ExprArg, Source, SourceTableId, TableFactor};

use super::{TableRef, TableWithJoins};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceTable {
    /// All tables referenced in the statement
    pub tables: Vec<TableRef>,

    /// The main table with joins
    pub from: Vec<TableWithJoins>,
}

impl SourceTable {
    pub fn new(tables: Vec<TableRef>, from: TableWithJoins) -> Self {
        Self {
            tables,
            from: vec![from],
        }
    }
}

impl From<ExprArg> for SourceTable {
    fn from(value: ExprArg) -> Self {
        SourceTable {
            tables: vec![TableRef::Arg(value)],
            from: vec![TableWithJoins {
                relation: TableFactor::Table(SourceTableId(0)),
                joins: vec![],
            }],
        }
    }
}

impl From<SourceTable> for Source {
    fn from(value: SourceTable) -> Self {
        Source::Table(value)
    }
}
