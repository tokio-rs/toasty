use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Keeps the input rows for which `predicate` holds.
///
/// The predicate is a function of one argument: `arg(0)` is the current row.
/// Every input row is read (the whole input must be scanned), so the input
/// is an [`Always`](mir::operation::InputRead::Always) read even though
/// evaluation is per-row.
///
/// Used when the database cannot apply all filter conditions natively (e.g.,
/// NoSQL drivers with limited query capabilities).
#[derive(Debug)]
pub(crate) struct Filter {
    /// The node producing the records to filter.
    pub(crate) input: mir::NodeId,

    /// The predicate: `arg(0)` = current row; keep the row when true.
    pub(crate) predicate: eval::Func,

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
            predicate: self.predicate.clone(),
        }
    }
}

impl From<Filter> for mir::Node {
    fn from(value: Filter) -> Self {
        mir::Operation::Filter(value).into()
    }
}
