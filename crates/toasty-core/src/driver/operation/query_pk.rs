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

    /// Maximum number of items to return. `None` means no limit.
    pub limit: Option<i64>,

    /// Sort key ordering direction for queries on a table with a composite
    /// primary key. `None` uses the driver's default ordering.
    pub order: Option<stmt::Direction>,

    /// Cursor for resuming a paginated query. Contains the serialized key of
    /// the last item from a previous page of results.
    pub cursor: Option<stmt::Value>,
}

impl From<QueryPk> for Operation {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}
