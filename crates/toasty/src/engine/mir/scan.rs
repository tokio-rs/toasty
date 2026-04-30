use indexmap::IndexSet;
use toasty_core::{
    driver::operation::QueryPkLimit,
    schema::db::{ColumnId, TableId},
    stmt,
};

use crate::engine::{exec, mir};

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
    pub(crate) limit: Option<QueryPkLimit>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl Scan {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &mir::LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Scan {
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

        exec::Scan {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            columns,
            row_filter: self.row_filter.clone(),
            limit: self.limit.clone(),
        }
    }
}

impl From<Scan> for mir::Node {
    fn from(value: Scan) -> Self {
        mir::Operation::Scan(value).into()
    }
}
