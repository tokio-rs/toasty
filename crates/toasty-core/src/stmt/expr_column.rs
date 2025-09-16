use super::Expr;
use crate::schema::db::ColumnId;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExprColumn {
    /// Which query the alias is listed in
    pub nesting: usize,

    /// The index of the alias in the `FROM` (or equivalent) clause
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
    pub fn new(nesting: usize, table: usize, column: usize) -> Self {
        Self { nesting, table, column }
    }

    /// Basic implementation that matches on column index
    /// This assumes single table context where column field corresponds to column_id.index
    /// TODO: This should be replaced with proper table context checking
    pub fn references(&self, column_id: ColumnId) -> bool {
        self.table == 0 && self.column == column_id.index
    }

    /// Basic implementation that assumes single table context
    /// This recreates a ColumnId from the column index, assuming table 0
    /// TODO: This should be replaced with proper table context resolution
    pub fn try_to_column_id(&self) -> Option<ColumnId> {
        // This method can only work reliably in simple single-table contexts
        // For now, return None for non-trivial cases to avoid creating invalid ColumnIds
        if self.nesting == 0 && self.table == 0 {
            // For single table context, we can reconstruct the ColumnId
            // Note: This assumes the table ID corresponds to the first table in the schema
            Some(ColumnId {
                table: crate::schema::db::TableId(0), // Assume first table
                index: self.column,
            })
        } else {
            None
        }
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Self::Column(value)
    }
}

// Temporary From implementation for backwards compatibility
// This assumes single table context (table 0) and uses column ID index as column index
// TODO: This should be replaced with proper table context tracking
impl From<ColumnId> for ExprColumn {
    fn from(value: ColumnId) -> Self {
        Self::new(0, 0, value.index)
    }
}

impl From<ColumnId> for Expr {
    fn from(value: ColumnId) -> Self {
        Self::column(value)
    }
}

