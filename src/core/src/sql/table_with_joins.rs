use super::*;

#[derive(Debug, Clone)]
pub struct TableWithJoins {
    /// Identify a table
    pub table: TableId,

    /// How the table will be referenced in the statement.
    pub alias: usize,
}
