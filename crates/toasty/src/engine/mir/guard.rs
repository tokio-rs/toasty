use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Gates a data-producing node with a boolean condition.
///
/// Evaluates the guard expression against the `guard_inputs`. If the guard
/// returns `true`, the `input` node's output is passed through unchanged. If
/// `false`, an empty list is produced instead, causing downstream operations
/// (e.g. `UpdateByKey`) to see no data and become a no-op.
#[derive(Debug)]
pub(crate) struct Guard {
    /// The node whose output is conditionally passed through.
    pub(crate) input: mir::NodeId,

    /// Nodes whose outputs are passed to `guard` for evaluation.
    pub(crate) guard_inputs: IndexSet<mir::NodeId>,

    /// Boolean expression evaluated against `guard_inputs`.
    pub(crate) guard: eval::Func,

    /// The output type (same as input).
    pub(crate) ty: stmt::Type,
}

impl Guard {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Guard {
        let input = logical_plan[self.input].var.get().unwrap();

        let guard_inputs = self
            .guard_inputs
            .iter()
            .map(|id| logical_plan[id].var.get().unwrap())
            .collect();

        let var = var_table.register_var(node.ty().clone());
        node.var.set(Some(var));

        exec::Guard {
            input,
            guard_inputs,
            output: exec::Output {
                var,
                num_uses: node.num_uses.get(),
            },
            guard: self.guard.clone(),
        }
    }
}

impl From<Guard> for mir::Node {
    fn from(value: Guard) -> Self {
        mir::Operation::Guard(value).into()
    }
}
