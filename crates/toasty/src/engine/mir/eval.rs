use indexmap::IndexSet;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Transforms records by applying a projection function.
///
/// Used to reshape records, extract specific fields, or compute derived values
/// from input records.
#[derive(Debug)]
pub(crate) struct Eval {
    /// The nodes providing parent and child data to merge.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The function to evaluate
    pub(crate) eval: eval::Func,

    /// The input from which meta-data should be forwarded. This includes the
    /// pagination cursors. When `None`, do not forward any metadata. Note, all
    /// other inputs must not have any metadata to forward.
    pub(crate) metadata: Option<usize>,
}

impl Eval {
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

        let output = var_table.register_var(self.eval.ret.clone());
        node.var.set(Some(output));

        exec::Eval {
            inputs: input_vars,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            eval: self.eval.clone(),
            metadata: self.metadata,
        }
    }
}

impl From<Eval> for mir::Node {
    fn from(value: Eval) -> Self {
        mir::Operation::Eval(value).into()
    }
}
