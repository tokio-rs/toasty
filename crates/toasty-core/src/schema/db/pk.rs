use super::*;

#[derive(Debug, PartialEq)]
pub struct PrimaryKey {
    /// Fields composing the primary key
    pub columns: Vec<ColumnId>,

    /// Primary key index
    pub index: IndexId,
}
