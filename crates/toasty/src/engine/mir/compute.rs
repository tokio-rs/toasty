use indexmap::IndexSet;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Evaluates `body` once over the whole outputs of `inputs`.
///
/// `arg(i)` in the body is `inputs[i]`'s complete output. For per-row
/// evaluation over one input's rows, use [`MapOver`](mir::MapOver) instead.
#[derive(Debug)]
pub(crate) struct Compute {
    /// The nodes whose outputs the body reads.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The function to evaluate: `arg(i)` = `inputs[i]`.
    pub(crate) body: eval::Func,
}

impl Compute {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Eval {
        let mut input_vars = vec![];

        for input in &self.inputs {
            let var = logical_plan[input].var.get().unwrap();
            input_vars.push(var);
        }

        let output = var_table.register_var(self.body.ret.clone());
        node.var.set(Some(output));

        exec::Eval {
            inputs: input_vars,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            eval: self.body.clone(),
            metadata: None,
        }
    }
}

impl From<Compute> for mir::Node {
    fn from(value: Compute) -> Self {
        mir::Operation::Compute(value).into()
    }
}
