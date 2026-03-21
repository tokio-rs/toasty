use crate::stmt::{ExprArg, Source, SourceTableId, TableFactor};

use super::{TableRef, TableWithJoins};

/// A lowered table-level data source for a `SELECT` statement.
///
/// Contains a list of table references (which may be schema tables, CTEs, or
/// derived tables) and the `FROM` items that reference them by index
/// ([`SourceTableId`]).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{SourceTable, TableRef, TableWithJoins, TableFactor, SourceTableId};
/// use toasty_core::schema::db::TableId;
///
/// let source = SourceTable::new(
///     vec![TableRef::Table(TableId(0))],
///     TableWithJoins {
///         relation: TableFactor::Table(SourceTableId(0)),
///         joins: vec![],
///     },
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SourceTable {
    /// All table references used in this source. Indexed by [`SourceTableId`].
    pub tables: Vec<TableRef>,

    /// The `FROM` items, each being a table with optional joins.
    pub from: Vec<TableWithJoins>,
}

impl SourceTable {
    /// Creates a new `SourceTable` with a single `FROM` item.
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
