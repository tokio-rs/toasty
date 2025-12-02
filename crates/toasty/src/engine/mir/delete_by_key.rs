use toasty_core::{schema::db::TableId, stmt};

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Deletes records by primary key.
///
/// Used with NoSQL drivers to delete records given a list of primary key values.
///
/// Keys are always specified as an input node, whether a [`Const`] or the
/// output of a dependent operation.
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// The node producing the list of primary keys to delete.
    pub(crate) input: mir::NodeId,

    /// The table to delete records from.
    pub(crate) table: TableId,

    /// Optional additional filter applied before deletion.
    pub(crate) filter: Option<stmt::Expr>,

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
        let input = logical_plan[self.input].var.get().unwrap();
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
        }
    }
}

impl From<DeleteByKey> for mir::Node {
    fn from(value: DeleteByKey) -> Self {
        mir::Operation::DeleteByKey(value).into()
    }
}
