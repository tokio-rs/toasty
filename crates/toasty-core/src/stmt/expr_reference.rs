use super::TableRef;
use crate::schema::{
    app::{FieldId, ModelId},
    db::TableId,
};
use std::fmt;

/// References to fields or columns at various nesting levels.
///
/// This enum supports two main use cases:
/// - App-level field references (before lowering to database schema)
/// - Database table/CTE column references (after lowering)
#[derive(Debug, Clone)]
pub enum ExprReference {
    /// Reference a field from a model, supporting nested query scopes.
    ///
    /// The `nesting` parameter indicates how many query levels up to look:
    /// - `0` = current statement
    /// - `1` = immediate parent query
    /// - `2` = grandparent query, etc.
    ///
    /// Note: The statement at the specified nesting level must be selecting
    /// from the model specified in the `model` field.
    Field {
        /// The model containing the field
        model: ModelId,
        /// Index of the field within the model
        index: usize,
        /// How many query levels up to find this field (0 = current level)
        nesting: usize,
    },

    /// Reference a column from a database table or CTE, used after lowering.
    ///
    /// When Field references are lowered to the database schema level,
    /// they become Column references. The `table` can reference either:
    /// - A regular database table (`TableRef::Table`)
    /// - A Common Table Expression (`TableRef::Cte`)
    Column {
        /// How many query levels up to find this column (0 = current level)
        nesting: usize,
        /// The table or CTE containing the column
        table: TableRef,
        /// Index of the column within the table/CTE
        index: usize,
    },
}

impl ExprReference {
    /// Create a field reference for current scope (nesting = 0)
    pub fn field(model: ModelId, index: usize) -> Self {
        Self::Field {
            model,
            index,
            nesting: 0,
        }
    }

    /// Create a field reference for parent scope
    pub fn parent_field(model: ModelId, index: usize, nesting: usize) -> Self {
        Self::Field {
            model,
            index,
            nesting,
        }
    }

    /// Create a column reference for a database table
    pub fn column(table: TableId, index: usize, nesting: usize) -> Self {
        Self::Column {
            nesting,
            table: TableRef::Table(table),
            index,
        }
    }

    /// Create a column reference for a CTE
    pub fn cte(nesting: usize, cte_index: usize, field_index: usize) -> Self {
        Self::Column {
            nesting,
            table: TableRef::Cte {
                nesting: 0,
                index: cte_index,
            },
            index: field_index,
        }
    }

    /// Set this reference to point to a specific field
    pub fn set_field(&mut self, field_id: FieldId) {
        *self = Self::Field {
            model: field_id.model,
            index: field_id.index,
            nesting: 0,
        };
    }

    /// Get the FieldId if this is a field reference with nesting = 0
    pub fn as_field_id(&self) -> Option<FieldId> {
        match self {
            Self::Field {
                model,
                index,
                nesting: 0,
            } => Some(FieldId {
                model: *model,
                index: *index,
            }),
            _ => None,
        }
    }

    /// Get the nesting level for any reference type
    pub fn nesting(&self) -> usize {
        match self {
            Self::Field { nesting, .. } | Self::Column { nesting, .. } => *nesting,
        }
    }
}

impl From<FieldId> for ExprReference {
    fn from(field_id: FieldId) -> Self {
        Self::Field {
            model: field_id.model,
            index: field_id.index,
            nesting: 0,
        }
    }
}

impl fmt::Display for ExprReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field {
                model,
                index,
                nesting,
            } => {
                if *nesting == 0 {
                    write!(f, "field({}, {index})", model.0)
                } else {
                    write!(f, "field({}, {index}, nest={nesting})", model.0)
                }
            }
            Self::Column {
                nesting,
                table,
                index,
            } => match table {
                TableRef::Table(table_id) => {
                    if *nesting == 0 {
                        write!(f, "column({}, {index})", table_id.0)
                    } else {
                        write!(f, "column({}, {index}, nest={nesting})", table_id.0)
                    }
                }
                TableRef::Cte {
                    nesting: _cte_nesting,
                    index: cte_index,
                } => {
                    if *nesting == 0 {
                        write!(f, "cte({cte_index}, {index})")
                    } else {
                        write!(f, "cte({cte_index}, {index}, nest={nesting})")
                    }
                }
            },
        }
    }
}
