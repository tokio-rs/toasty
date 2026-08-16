use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Produces `value` once per row of `input`.
///
/// The input supplies only a cardinality — its rows are never read, and a
/// write that reports just an affected-row count works the same as one that
/// returns rows. Used for a returning clause whose value is fully known
/// client-side (e.g. an UPDATE whose assignments were all literals): the
/// database is not asked for any columns, and the result is the value
/// repeated once per affected row.
#[derive(Debug)]
pub(crate) struct Repeat {
    /// The node whose cardinality drives the repetition.
    pub(crate) input: mir::NodeId,

    /// The value produced for each input row.
    pub(crate) value: stmt::Value,

    /// Output type: `List<value's type>`.
    pub(crate) ty: stmt::Type,
}

impl Repeat {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Repeat {
        let input_var = logical_plan[self.input].var.get().unwrap();

        let var = var_table.register_var(self.ty.clone());
        node.var.set(Some(var));

        exec::Repeat {
            input: input_var,
            output: exec::Output {
                var,
                num_uses: node.num_uses.get(),
            },
            value: self.value.clone(),
        }
    }
}

impl From<Repeat> for mir::Node {
    fn from(value: Repeat) -> Self {
        mir::Operation::Repeat(value).into()
    }
}
