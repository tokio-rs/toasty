use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

#[derive(Debug)]
pub(crate) struct Project {
    /// Input required to perform the projection
    pub(crate) input: mir::NodeId,

    /// Projection expression
    pub(crate) projection: eval::Func,

    pub(crate) ty: stmt::Type,
}

impl Project {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Project {
        let input_var = logical_plan[self.input].var.get().unwrap();

        let var = var_table.register_var(stmt::Type::list(self.projection.ret.clone()));
        node.var.set(Some(var));

        exec::Project {
            input: input_var,
            output: exec::Output {
                var,
                num_uses: node.num_uses.get(),
            },
            projection: self.projection.clone(),
        }
    }
}

impl From<Project> for mir::Node {
    fn from(value: Project) -> Self {
        mir::Operation::Project(value).into()
    }
}
