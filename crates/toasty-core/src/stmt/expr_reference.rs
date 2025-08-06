use crate::schema::app::{FieldId, ModelId};
use std::fmt;

#[derive(Debug, Clone)]
pub enum ExprReference {
    /// Reference a field from a model
    Field { model: ModelId, index: usize },

    /// Reference a column from a CTE table
    Cte {
        /// What level of nesting the reference is compared to the CTE being
        /// referenced.
        nesting: usize,

        /// Column index in the CTEs
        index: usize,
    },
}

impl ExprReference {
    pub fn field(model: ModelId, index: usize) -> Self {
        Self::Field { model, index }
    }

    pub fn cte(nesting: usize, index: usize) -> Self {
        Self::Cte { nesting, index }
    }

    /// Set this reference to point to a specific field
    pub fn set_field(&mut self, field_id: FieldId) {
        *self = Self::Field {
            model: field_id.model,
            index: field_id.index,
        };
    }

    /// Get the FieldId if this is a field reference
    pub fn as_field_id(&self) -> Option<FieldId> {
        match self {
            Self::Field { model, index } => Some(FieldId {
                model: *model,
                index: *index,
            }),
            Self::Cte { .. } => None,
        }
    }
}

impl From<FieldId> for ExprReference {
    fn from(field_id: FieldId) -> Self {
        Self::Field {
            model: field_id.model,
            index: field_id.index,
        }
    }
}

impl fmt::Display for ExprReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field { model, index } => write!(f, "field({}, {index})", model.0),
            Self::Cte { nesting, index } => write!(f, "cte({nesting}, {index})"),
        }
    }
}
