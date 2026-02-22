use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Merges child records into parent records.
///
/// Used to combine the results of parent and child queries when loading
/// associations (e.g., users with their todos). The merge produces nested
/// records where each parent contains its associated children.
#[derive(Debug)]
pub(crate) struct NestedMerge {
    /// The nodes providing parent and child data to merge.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// Configuration for how to perform the merge at each nesting level.
    pub(crate) root: exec::NestedLevel,

    /// Flat list of hash indexes to build before the merge, computed at plan time.
    pub(crate) hash_indexes: Vec<exec::MergeIndex>,

    /// Flat list of sorted indexes to build before the merge, computed at plan time.
    pub(crate) sort_indexes: Vec<exec::MergeIndex>,
}

impl NestedMerge {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::NestedMerge {
        let mut input_vars = vec![];

        for input in &self.inputs {
            let var = logical_plan[input].var.get().unwrap();
            input_vars.push(var);
        }

        let output = var_table.register_var(stmt::Type::list(self.root.projection.ret.clone()));
        node.var.set(Some(output));

        exec::NestedMerge {
            inputs: input_vars,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            root: self.root.clone(),
            hash_indexes: self.hash_indexes.clone(),
            sort_indexes: self.sort_indexes.clone(),
        }
    }
}

impl From<NestedMerge> for mir::Node {
    fn from(value: NestedMerge) -> Self {
        mir::Operation::NestedMerge(value).into()
    }
}
