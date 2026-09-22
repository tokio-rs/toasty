use indexmap::IndexSet;
use toasty_core::{driver::operation::Pagination, schema::db::TableId, stmt};

use crate::engine::mir;

/// Performs a full-table scan with optional filter, limit, and pagination.
///
/// `Scan` is emitted by the planner when no index covers the query filter on a
/// DynamoDB-backed model. The driver applies `row_filter` to each scanned row
/// before returning results.
#[derive(Debug)]
pub(crate) struct Scan {
    /// Optional node providing input arguments for the filter expression.
    pub(crate) input: Option<mir::NodeId>,

    /// The table to scan.
    pub(crate) table: TableId,

    /// The columns to include in the returned records.
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// Filter expression applied to each scanned row.
    pub(crate) row_filter: Option<stmt::Expr>,

    /// Limit and pagination bounds. `None` means return all rows.
    pub(crate) limit: Option<Pagination>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl From<Scan> for mir::Node {
    fn from(value: Scan) -> Self {
        mir::Operation::Scan(value).into()
    }
}
