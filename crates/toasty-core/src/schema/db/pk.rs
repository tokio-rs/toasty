use super::{ColumnId, IndexId};

/// The primary key definition for a database table.
///
/// Lists the columns that make up the primary key and references the
/// corresponding [`Index`](super::Index).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{PrimaryKey, ColumnId, IndexId, TableId};
///
/// let pk = PrimaryKey {
///     columns: vec![
///         ColumnId { table: TableId(0), index: 0 },
///     ],
///     index: IndexId { table: TableId(0), index: 0 },
/// };
///
/// assert_eq!(pk.columns.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimaryKey {
    /// Columns composing the primary key, in order.
    pub columns: Vec<ColumnId>,

    /// The index backing this primary key.
    pub index: IndexId,
}
