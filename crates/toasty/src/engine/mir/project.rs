use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Transforms records by applying a projection function.
///
/// Used to reshape records, extract specific fields, or compute derived values
/// from input records.
#[derive(Debug)]
pub(crate) struct Project {
    /// The node producing the records to transform.
    pub(crate) input: mir::NodeId,

    /// The projection function mapping input records to output records.
    pub(crate) projection: eval::Func,

    /// The output type after projection.
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
