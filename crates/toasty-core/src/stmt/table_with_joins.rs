use super::{Join, TableFactor};

/// A `FROM` item: a table reference paired with zero or more joins.
///
/// Represents one entry in the `FROM` clause of a `SELECT` statement at the
/// table level.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{TableWithJoins, TableFactor, SourceTableId};
///
/// let twj = TableWithJoins {
///     relation: TableFactor::Table(SourceTableId(0)),
///     joins: vec![],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TableWithJoins {
    /// The base table or derived table.
    pub relation: TableFactor,

    /// Joins applied to the base relation.
    pub joins: Vec<Join>,
}
