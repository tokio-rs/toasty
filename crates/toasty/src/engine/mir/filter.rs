use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Applies an in-memory filter predicate to a list of records.
///
/// Used when the database cannot apply all filter conditions natively (e.g.,
/// NoSQL drivers with limited query capabilities).
#[derive(Debug)]
pub(crate) struct Filter {
    /// The node producing the records to filter.
    pub(crate) input: mir::NodeId,

    /// The predicate function to apply to each record.
    pub(crate) filter: eval::Func,

    /// The output type (same as input, but potentially fewer rows).
    pub(crate) ty: stmt::Type,
}

impl Filter {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Filter {
        let input = logical_plan[self.input].var.get().unwrap();
        let ty = node.ty().clone();

        let var = var_table.register_var(ty);
        node.var.set(Some(var));

        exec::Filter {
            input,
            output: exec::Output {
                var,
                num_uses: node.num_uses.get(),
            },
            filter: self.filter.clone(),
        }
    }
}

impl From<Filter> for mir::Node {
    fn from(value: Filter) -> Self {
        mir::Operation::Filter(value).into()
    }
}
