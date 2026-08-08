use indexmap::IndexSet;
use toasty_core::{schema::db::TableId, stmt};

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Deletes records by primary key.
///
/// Used with NoSQL drivers to delete records given a list of primary key values.
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// Input nodes. The first produces the list of primary keys to delete,
    /// whether a [`Const`] or the output of a dependent operation. `Arg`
    /// positions in `filter` and `condition` index into this list.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The table to delete records from.
    pub(crate) table: TableId,

    /// Optional additional filter applied before deletion.
    pub(crate) filter: Option<stmt::Expr>,

    /// Optional condition for optimistic locking (e.g., version check).
    pub(crate) condition: Option<stmt::Expr>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl DeleteByKey {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::DeleteByKey {
        let input = self
            .inputs
            .iter()
            .map(|node_id| logical_plan[node_id].var.get().unwrap())
            .collect();
        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        exec::DeleteByKey {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            filter: self.filter.clone(),
            condition: self.condition.clone(),
        }
    }
}

impl From<DeleteByKey> for mir::Node {
    fn from(value: DeleteByKey) -> Self {
        mir::Operation::DeleteByKey(value).into()
    }
}
