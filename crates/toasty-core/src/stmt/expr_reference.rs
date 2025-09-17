use crate::{schema::app::FieldId, stmt::Expr};

#[derive(Debug, Clone)]
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

    /// Reference a column from a CTE table
    Cte {
        /// What level of nesting the reference is compared to the CTE being
        /// referenced.
        nesting: usize,

        /// Column index in the CTEs
        index: usize,
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
}

impl ExprReference {
    pub fn field(field: impl Into<FieldId>) -> Self {
        ExprReference::Field {
            nesting: 0,
            index: field.into().index,
        }
    }
}
