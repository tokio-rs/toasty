use indexmap::IndexSet;
use toasty_core::{
    driver::operation::Pagination,
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::engine::mir;

/// Queries records using a primary key filter.
///
/// Used with NoSQL drivers to query a table's primary key index with optional
/// additional row filtering.
#[derive(Debug)]
pub(crate) struct QueryPk {
    /// Optional node providing input arguments for the filter.
    pub(crate) input: Option<mir::NodeId>,

    /// The table to query.
    pub(crate) table: TableId,

    /// Optional index to query. None = primary key, Some(id) = secondary index
    pub(crate) index: Option<IndexId>,

    /// The columns to include in the returned records.
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// Filter expression for the primary key index.
    pub(crate) pk_filter: stmt::Expr,

    /// Additional filter applied to matching rows.
    pub(crate) row_filter: Option<stmt::Expr>,

    /// The return type.
    pub(crate) ty: stmt::Type,

    /// Limit and pagination bounds for this query. `None` means unbounded.
    pub(crate) limit: Option<Pagination>,

    /// Sort key ordering direction.
    pub(crate) order: Option<stmt::Direction>,
}

impl From<QueryPk> for mir::Node {
    fn from(value: QueryPk) -> Self {
        mir::Operation::QueryPk(value).into()
    }
}
