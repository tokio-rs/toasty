use super::SourceTableId;

#[derive(Debug, Clone, PartialEq)]
pub enum TableFactor {
    /// Reference to a table in the SourceTable's tables vec
    Table(SourceTableId),
}
