use crate::{
    schema::{app::FieldId, db::ColumnId},
    stmt::Expr,
};

/// A reference to a model, field, or column.
///
/// References use scope-based nesting to support subqueries. A nesting level of
/// `0` refers to the current query scope, while higher levels reference higher
/// scope queries.
///
/// # Examples
///
/// ```text
/// ref(field: 0, nesting: 0)  // field 0 in current query
/// ref(field: 2, nesting: 1)  // field 2 in parent query
/// ref(column: 0, table: 1)   // column 0 in table 1
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub enum ExprReference {
    /// A reference to a column in a database-level statement.
    ///
    /// ExprReference::Column represents resolved column references after lowering from the
    /// application schema to the database schema. It uses a scope-based approach
    /// similar to ExprReference::Field, referencing a specific column within a target
    /// at a given nesting level.
    Column(ExprColumn),

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

    /// Reference a model at a specific nesting level.
    ///
    /// This is roughly referencing the full record instead of a specific field.
    Model { nesting: usize },
}

/// A reference to a database column.
///
/// Used after lowering from the application schema to the database schema.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct ExprColumn {
    /// Query scope nesting level: `0` = current query, `1`+ = higher scope queries.
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
    pub fn is_expr_reference(&self) -> bool {
        matches!(self, Expr::Reference(..))
    }

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
    pub fn ref_self_field(field: impl Into<FieldId>) -> Self {
        ExprReference::field(field).into()
    }

    /// Create a reference to a field at a specified nesting level
    pub fn ref_field(nesting: usize, field: impl Into<FieldId>) -> Self {
        ExprReference::Field {
            nesting,
            index: field.into().index,
        }
        .into()
    }

    /// Create a reference to a field one level up
    pub fn ref_parent_field(field: impl Into<FieldId>) -> Self {
        ExprReference::Field {
            nesting: 1,
            index: field.into().index,
        }
        .into()
    }

    pub fn is_field(&self) -> bool {
        matches!(self, Self::Reference(ExprReference::Field { .. }))
    }

    /// Create a model reference to the parent model
    pub fn ref_parent_model() -> Self {
        Self::ref_ancestor_model(1)
    }

    /// Create a model reference to the specified nesting level
    pub fn ref_ancestor_model(nesting: usize) -> Self {
        ExprReference::Model { nesting }.into()
    }

    pub fn column(column: impl Into<ExprReference>) -> Self {
        column.into().into()
    }

    pub fn is_column(&self) -> bool {
        matches!(self, Self::Reference(ExprReference::Column(..)))
    }

    pub fn as_expr_reference(&self) -> Option<&ExprReference> {
        match self {
            Expr::Reference(expr_reference) => Some(expr_reference),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_expr_reference_unwrap(&self) -> &ExprReference {
        self.as_expr_reference()
            .unwrap_or_else(|| panic!("expected ExprReference; actual={self:#?}"))
    }

    pub fn as_expr_column(&self) -> Option<&ExprColumn> {
        match self {
            Expr::Reference(ExprReference::Column(expr_column)) => Some(expr_column),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_expr_column_unwrap(&self) -> &ExprColumn {
        self.as_expr_column()
            .unwrap_or_else(|| panic!("expected ExprColumn; actual={self:#?}"))
    }
}

impl ExprReference {
    pub fn field(field: impl Into<FieldId>) -> Self {
        ExprReference::Field {
            nesting: 0,
            index: field.into().index,
        }
    }

    pub fn is_field(&self) -> bool {
        matches!(self, ExprReference::Field { .. })
    }

    pub fn is_model(&self) -> bool {
        matches!(self, ExprReference::Model { .. })
    }

    pub fn column(table: usize, column: usize) -> Self {
        ExprReference::Column(ExprColumn {
            nesting: 0,
            table,
            column,
        })
    }

    pub fn is_column(&self) -> bool {
        matches!(self, ExprReference::Column(..))
    }

    pub fn as_expr_column(&self) -> Option<&ExprColumn> {
        match self {
            ExprReference::Column(expr_column) => Some(expr_column),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_expr_column_unwrap(&self) -> &ExprColumn {
        match self {
            ExprReference::Column(expr_column) => expr_column,
            _ => panic!("expected ExprColumn; actual={self:#?}"),
        }
    }
}

impl From<ExprColumn> for ExprReference {
    fn from(value: ExprColumn) -> Self {
        ExprReference::Column(value)
    }
}

impl From<ExprReference> for Expr {
    fn from(value: ExprReference) -> Self {
        Expr::Reference(value)
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Expr::Reference(ExprReference::Column(value))
    }
}

impl From<&ExprReference> for Expr {
    fn from(value: &ExprReference) -> Self {
        Expr::Reference(*value)
    }
}

impl From<&ExprColumn> for Expr {
    fn from(value: &ExprColumn) -> Self {
        Expr::Reference(ExprReference::Column(*value))
    }
}

impl From<ColumnId> for ExprReference {
    fn from(value: ColumnId) -> Self {
        ExprReference::Column(value.into())
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
