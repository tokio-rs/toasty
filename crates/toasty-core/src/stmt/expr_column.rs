use super::Expr;
use crate::schema::db::ColumnId;

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

    /// Create a new ExprColumn reference with a specific nesting level.
    pub fn with_nesting(nesting: usize, table: usize, column: usize) -> Self {
        ExprColumn {
            nesting,
            table,
            column,
        }
    }

    /// Check if this ExprColumn references a specific ColumnId.
    ///
    /// Note: This is a transitional method that works during the migration from ColumnId-based
    /// to struct-based ExprColumn. The accuracy depends on how the ExprColumn was created.
    // pub fn references(&self, column_id: ColumnId) -> bool {
    //     // For ExprColumn created from ColumnId via our From<ColumnId> implementation,
    //     // we can try to match by reconstructing the original ColumnId
    //     if self.nesting == 0 {
    //         // Check if both table and column indices match
    //         if self.table == column_id.table.0 && self.column == column_id.index {
    //             true
    //         } else if self.table == 0 {
    //             // Legacy case where table=0 was used as fallback
    //             // Just match on column index for backward compatibility
    //             self.column == column_id.index
    //         } else {
    //             false
    //         }
    //     } else {
    //         // For ExprColumn with nesting > 0, we can't reliably determine this without more context
    //         false
    //     }
    // }

    /// Try to convert this ExprColumn back to a ColumnId.
    ///
    /// This is a transitional method that works for simple cases where
    /// the ExprColumn was created from a ColumnId via our transition helper.
    /// Returns None for cases that require statement context or cannot be resolved.
    pub fn try_to_column_id(&self) -> Option<ColumnId> {
        // Only handle the simple case where nesting=0 and table=0
        // This likely came from our From<ColumnId> transition helper
        if self.nesting == 0 && self.table == 0 {
            // For now, return None to avoid creating invalid ColumnIds
            // The proper solution requires more context about the statement and schema
            // TODO: Implement proper ColumnId reconstruction with schema context
            None
        } else {
            // For more complex references, we need statement context
            None
        }
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Self::Column(value)
    }
}

// Temporary transition helper - will be removed once all ColumnId usages are updated
impl From<ColumnId> for ExprColumn {
    fn from(value: ColumnId) -> Self {
        // Try to preserve table information by using the TableId index
        // This assumes that TableId(n) corresponds to table index n in most cases
        // This is a heuristic and may not always be correct
        ExprColumn {
            nesting: 0,
            table: value.table.0, // Use TableId index as table index
            column: value.index,
        }
    }
}
