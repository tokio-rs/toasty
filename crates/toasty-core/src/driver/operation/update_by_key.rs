use super::Operation;

use crate::{
    schema::db::{ColumnId, TableId},
    stmt,
};

/// Updates one or more records identified by primary key.
///
/// Used by key-value drivers. SQL drivers receive an equivalent `UPDATE`
/// statement via [`QuerySql`](super::QuerySql) instead. Supports conditional
/// updates and optionally returns the updated records.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{UpdateByKey, Operation};
///
/// let op = UpdateByKey {
///     table: table_id,
///     keys: vec![key_value],
///     assignments: assignments,
///     filter: None,
///     condition: None,
///     returning: None,
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct UpdateByKey {
    /// The table to update.
    pub table: TableId,

    /// Primary key values identifying the records to update.
    pub keys: Vec<stmt::Value>,

    /// Column assignments describing how to modify the records.
    pub assignments: stmt::Assignments,

    /// Optional filter expression. When set, only records whose key is in
    /// `keys` *and* that match this filter are updated.
    pub filter: Option<stmt::Expr>,

    /// Optional precondition that must hold for the update to be applied.
    /// Unlike `filter`, a failed condition typically causes an error rather
    /// than silently skipping the row.
    pub condition: Option<stmt::Expr>,

    /// The columns to return for each updated row.
    ///
    /// `None` returns the affected-row count. `Some(columns)` returns one
    /// record per updated row containing exactly these columns, in this order,
    /// in the [`ExecResponse`](super::super::ExecResponse). The engine builds
    /// this list explicitly, so the driver never has to infer which columns to
    /// return from the assignments.
    pub returning: Option<Vec<ColumnId>>,
}

impl From<UpdateByKey> for Operation {
    fn from(value: UpdateByKey) -> Self {
        Self::UpdateByKey(value)
    }
}
