use super::*;

#[derive(Debug, Clone)]
pub struct TableWithJoins {
    /// Identify a table
    pub table: TableRef,

    /// Joins to apply
    pub joins: Vec<Join>,
}
