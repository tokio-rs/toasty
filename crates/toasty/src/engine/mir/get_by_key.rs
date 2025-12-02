use indexmap::IndexSet;
use toasty_core::{
    schema::db::{ColumnId, TableId},
    stmt,
};

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Batch fetches records by primary key.
///
/// Used with NoSQL drivers to retrieve multiple records given a list of
/// primary key values.
///
/// Keys are always specified as an input node, whether a [`Const`] or the
/// output of a dependent operation.
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// The node producing the list of primary keys to fetch.
    pub(crate) input: mir::NodeId,

    /// The table to fetch records from.
    pub(crate) table: TableId,

    /// The columns to include in the returned records.
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl GetByKey {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::GetByKey {
        let input = logical_plan[self.input].var.get().unwrap();

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

        exec::GetByKey {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            columns,
        }
    }
}

impl From<GetByKey> for mir::Node {
    fn from(value: GetByKey) -> Self {
        mir::Operation::GetByKey(value).into()
    }
}
