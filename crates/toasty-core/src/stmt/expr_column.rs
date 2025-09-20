use crate::schema::db::ColumnId;

use super::Expr;

/// A reference to a column in a database-level statement.
///
/// ExprColumn represents resolved column references after lowering from the
/// application schema to the database schema. It uses a scope-based approach
/// similar to ExprReference, referencing a specific column within a target
/// at a given nesting level.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExprColumn {
    /// Query scope nesting level: 0 = current query, 1+ = higher scope queries
    pub nesting: usize,

    /// Index into the table references vector for this column's source relation.
    ///
    /// For statements with multiple tables (SELECT with JOINs), this indexes into
    /// the `SourceTable::tables` field to identify which specific table contains
    /// this column. For single-target statements (INSERT, UPDATE), this is
    /// typically 0 since these operations target only one relation at a time.
    pub table: usize,

    /// The index of the column in the table
    pub column: usize,
}

impl Expr {
    pub fn column(column: impl Into<ExprColumn>) -> Self {
        column.into().into()
    }

    pub fn is_column(&self) -> bool {
        matches!(self, Self::Column(_))
    }
}

impl ExprColumn {
    /// Create a new ExprColumn reference to a column in the current query scope.
    pub fn new(table: usize, column: usize) -> Self {
        ExprColumn {
            nesting: 0,
            table,
            column,
        }
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Self::Column(value)
    }
}

impl From<ColumnId> for ExprColumn {
    fn from(value: ColumnId) -> Self {
        ExprColumn {
            nesting: 0,
            table: 0,
            column: value.index,
        }
    }
}
