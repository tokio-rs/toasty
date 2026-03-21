use super::Operation;

use crate::{
    schema::db::{ColumnId, TableId},
    stmt,
};

/// Fetches one or more records by exact primary key match.
///
/// This is the key-value equivalent of `SELECT ... WHERE pk IN (...)`. The
/// driver returns a row for each key that exists in the table.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{GetByKey, Operation};
///
/// let op = GetByKey {
///     table: table_id,
///     select: vec![col_id_a, col_id_b],
///     keys: vec![key1, key2],
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct GetByKey {
    /// The table to fetch from.
    pub table: TableId,

    /// Which columns to include in the returned rows.
    pub select: Vec<ColumnId>,

    /// Primary key values identifying the records to fetch.
    pub keys: Vec<stmt::Value>,
}

impl From<GetByKey> for Operation {
    fn from(value: GetByKey) -> Self {
        Self::GetByKey(value)
    }
}
