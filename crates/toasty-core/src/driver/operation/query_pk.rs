use super::Operation;
use crate::{
    schema::db::{ColumnId, IndexId, TableId},
    stmt,
};

#[derive(Debug, Clone)]
pub struct QueryPk {
    /// Table to query
    pub table: TableId,

    /// Optional index to query. None = primary key, Some(id) = secondary index
    pub index: Option<IndexId>,

    /// Which columns to get
    pub select: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub filter: Option<stmt::Expr>,

    /// Maximum number of items to evaluate. Maps to DynamoDB's `Limit`
    /// parameter. `None` means no limit (return all matching items).
    pub limit: Option<i64>,

    /// Controls sort key ordering for queries on a table with a composite
    /// primary key. `true` means ascending (default DynamoDB behavior), `false`
    /// means descending. Maps to DynamoDB's `ScanIndexForward` parameter.
    pub scan_index_forward: Option<bool>,

    /// Cursor for resuming a paginated query. Contains the serialized primary
    /// key of the last evaluated item from a previous query response. Maps to
    /// DynamoDB's `ExclusiveStartKey` parameter.
    pub exclusive_start_key: Option<stmt::Value>,
}

impl From<QueryPk> for Operation {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}
