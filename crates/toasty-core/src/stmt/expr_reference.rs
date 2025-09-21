use crate::{
    schema::{app::FieldId, db::ColumnId},
    stmt::Expr,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ExprReference {
    /// Reference a specific field in a query's relation.
    ///
    /// For Query/Delete statements, the relation is the Source.
    /// For Insert/Update statements, the relation is the target.
    Field {
        /// Query scope nesting level: 0 = current query, 1+ = higher scope queries
        nesting: usize,
        /// Index of the field within the relation
        index: usize,
    },

    /// A reference to a column in a database-level statement.
    ///
    /// ExprReference::Column represents resolved column references after lowering from the
    /// application schema to the database schema. It uses a scope-based approach
    /// similar to ExprReference::Field, referencing a specific column within a target
    /// at a given nesting level.
    Column {
        /// Query scope nesting level: 0 = current query, 1+ = higher scope queries
        nesting: usize,

        /// Index into the table references vector for this column's source relation.
        ///
        /// For statements with multiple tables (SELECT with JOINs), this indexes into
        /// the `SourceTable::tables` field to identify which specific table contains
        /// this column. For single-target statements (INSERT, UPDATE), this is
        /// typically 0 since these operations target only one relation at a time.
        table: usize,

        /// The index of the column in the table
        column: usize,
    },
}

impl Expr {
    /// Creates an expression that references a field in the current query.
    ///
    /// This creates an `ExprReference::Field` with `nesting = 0`, meaning it
    /// references a field in the current query scope rather than an outer query.
    ///
    /// # Arguments
    ///
    /// * `field` - A field identifier that can be converted into a `FieldId`
    ///
    /// # Returns
    ///
    /// An `Expr::Reference` containing an `ExprReference::Field` that points to
    /// the specified field in the current query's relation.
    pub fn field(field: impl Into<FieldId>) -> Self {
        ExprReference::field(field).into()
    }

    pub fn is_field(&self) -> bool {
        matches!(self, Self::Reference(ExprReference::Field { .. }))
    }

    pub fn column(column: impl Into<ExprReference>) -> Self {
        column.into().into()
    }

    pub fn is_column(&self) -> bool {
        matches!(self, Self::Reference(ExprReference::Column { .. }))
    }
}

impl ExprReference {
    pub fn field(field: impl Into<FieldId>) -> Self {
        ExprReference::Field {
            nesting: 0,
            index: field.into().index,
        }
    }

    pub fn column(table: usize, column: usize) -> Self {
        ExprReference::Column {
            nesting: 0,
            table,
            column,
        }
    }
}

impl From<ColumnId> for ExprReference {
    fn from(value: ColumnId) -> Self {
        ExprReference::Column {
            nesting: 0,
            table: 0,
            column: value.index,
        }
    }
}
