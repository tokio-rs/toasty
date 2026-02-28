use indexmap::IndexSet;
use toasty_core::{
    schema::db::{ColumnId, IndexId, TableId},
    stmt,
};

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

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

    /// Maximum number of items to evaluate.
    pub(crate) limit: Option<i64>,

    /// Sort key ordering (`true` = ascending, `false` = descending).
    pub(crate) scan_index_forward: Option<bool>,

    /// Cursor for resuming a paginated query.
    pub(crate) exclusive_start_key: Option<stmt::Value>,
}

impl QueryPk {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::QueryPk {
        let input = self
            .input
            .map(|node_id| logical_plan[node_id].var.get().unwrap());
        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        let columns = self
            .columns
            .iter()
            .map(|expr_reference| {
                let stmt::ExprReference::Column(expr_column) = expr_reference else {
                    todo!()
                };
                debug_assert_eq!(expr_column.nesting, 0);
                debug_assert_eq!(expr_column.table, 0);

                ColumnId {
                    table: self.table,
                    index: expr_column.column,
                }
            })
            .collect();

        exec::QueryPk {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            index: self.index,
            columns,
            pk_filter: self.pk_filter.clone(),
            row_filter: self.row_filter.clone(),
            limit: self.limit,
            scan_index_forward: self.scan_index_forward,
            exclusive_start_key: self.exclusive_start_key.clone(),
        }
    }
}

impl From<QueryPk> for mir::Node {
    fn from(value: QueryPk) -> Self {
        mir::Operation::QueryPk(value).into()
    }
}
