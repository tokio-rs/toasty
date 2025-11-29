use indexmap::IndexSet;
use toasty_core::{
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Finds primary keys via a secondary index lookup.
///
/// Used with NoSQL drivers to locate records by a secondary index, returning
/// the primary keys which can then be used with [`GetByKey`].
#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    /// Nodes providing input arguments for the filter.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The table containing the index.
    pub(crate) table: TableId,

    /// The secondary index to query.
    pub(crate) index: IndexId,

    /// Filter expression for the index.
    pub(crate) filter: stmt::Expr,

    /// The return type (a list of primary keys).
    pub(crate) ty: stmt::Type,
}

impl FindPkByIndex {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::FindPkByIndex {
        let input = self
            .inputs
            .iter()
            .map(|node_id| logical_plan[node_id].var.get().unwrap())
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
