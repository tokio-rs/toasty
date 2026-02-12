use super::{ColumnId, IndexId};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimaryKey {
    /// Fields composing the primary key
    pub columns: Vec<ColumnId>,

    /// Primary key index
    pub index: IndexId,
}
