use super::Operation;
use crate::{schema::db::TableId, stmt};

/// Deletes one or more records from a table by primary key.
///
/// Used by key-value drivers (e.g., DynamoDB). SQL drivers receive an
/// equivalent `DELETE` statement via [`QuerySql`](super::QuerySql) instead.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{DeleteByKey, Operation};
///
/// let op = DeleteByKey {
///     table: table_id,
///     keys: vec![key_value],
///     filter: None,
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct DeleteByKey {
    /// The table to delete from.
    pub table: TableId,

    /// Primary key values identifying the records to delete.
    pub keys: Vec<stmt::Value>,

    /// Optional filter expression. When set, only records whose key is in
    /// `keys` *and* that match this filter are deleted.
    pub filter: Option<stmt::Expr>,

    /// Optional condition for optimistic locking. When set, the delete fails
    /// with an error if the condition is not met (unlike `filter`, which
    /// silently skips non-matching records).
    pub condition: Option<stmt::Expr>,
}

impl From<DeleteByKey> for Operation {
    fn from(value: DeleteByKey) -> Self {
        Self::DeleteByKey(value)
    }
}
