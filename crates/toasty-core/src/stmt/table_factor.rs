use super::SourceTableId;

/// A table reference within a [`TableWithJoins`](super::TableWithJoins) relation.
///
/// Currently only supports direct table references via [`SourceTableId`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{TableFactor, SourceTableId};
///
/// let factor = TableFactor::Table(SourceTableId(0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TableFactor {
    /// A reference to a table in the [`SourceTable::tables`](super::SourceTable) vector.
    Table(SourceTableId),
}
