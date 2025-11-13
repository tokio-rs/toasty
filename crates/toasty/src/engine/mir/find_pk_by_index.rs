use indexmap::IndexSet;
use toasty_core::{
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::engine::{exec, mir, planner::VarTable};

#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    pub(crate) inputs: IndexSet<mir::NodeId>,
    pub(crate) table: TableId,
    pub(crate) index: IndexId,
    pub(crate) filter: stmt::Expr,
    pub(crate) ty: stmt::Type,
}

impl FindPkByIndex {
    pub(crate) fn to_exec(
        &self,
        graph: &mir::Store,
        node: &mir::Node,
        var_table: &mut VarTable,
    ) -> exec::FindPkByIndex {
        let input = self
            .inputs
            .iter()
            .map(|node_id| graph.var_id(*node_id))
            .collect();

        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        exec::FindPkByIndex {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            index: self.index,
            filter: self.filter.clone(),
        }
    }
}

impl From<FindPkByIndex> for mir::Node {
    fn from(value: FindPkByIndex) -> Self {
        mir::Operation::FindPkByIndex(value).into()
    }
}
