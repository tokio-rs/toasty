// TODO: Remove this file. This should be implementable with [`stmt::Insert`], however for
// migrations we need to reference table names which are not part of the schema, and that
// is currently not implemented.

use super::{Name, Statement};

/// A statement to copy rows from one table to another.
///
/// Generates: `INSERT INTO "target" ("t_col1", "t_col2") SELECT "s_col1", "s_col2" FROM "source"`
#[derive(Debug, Clone)]
pub struct CopyTable {
    /// Source table name.
    pub source: Name,

    /// Target table name.
    pub target: Name,

    /// Column mappings: (target_column_name, source_column_name).
    pub columns: Vec<(Name, Name)>,
}

impl Statement {
    /// Creates a statement that copies rows from one table to another.
    pub fn copy_table(source: Name, target: Name, columns: Vec<(Name, Name)>) -> Self {
        CopyTable {
            source,
            target,
            columns,
        }
        .into()
    }
}

impl From<CopyTable> for Statement {
    fn from(value: CopyTable) -> Self {
        Self::CopyTable(value)
    }
}
