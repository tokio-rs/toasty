use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

#[derive(Debug)]
pub(crate) struct Filter {
    /// Input needed to reify the statement
    pub(crate) input: mir::NodeId,

    /// Filter
    pub(crate) filter: eval::Func,

    /// Row type
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
