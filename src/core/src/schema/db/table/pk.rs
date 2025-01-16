use super::*;

#[derive(Debug, PartialEq)]
pub struct TablePrimaryKey {
    /// Fields composing the primary key
    pub columns: Vec<ColumnId>,

    /// Primary key index
    pub index: IndexId,
}
