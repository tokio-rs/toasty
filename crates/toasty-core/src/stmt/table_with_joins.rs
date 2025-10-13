use super::{Join, TableFactor};

#[derive(Debug, Clone, PartialEq)]
pub struct TableWithJoins {
    /// The table relation
    pub relation: TableFactor,

    /// Joins to apply
    pub joins: Vec<Join>,
}
