use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The read statement
    pub(crate) read: stmt::Query,

    /// The write statement
    pub(crate) write: stmt::Statement,

    /// Node return type
    pub(crate) ty: stmt::Type,
}

impl ReadModifyWrite {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::ReadModifyWrite {
        let input = self
            .inputs
            .iter()
            .map(|input| logical_plan[input].var.get().unwrap())
            .collect();

        // A hack since rmw doesn't support output yet
        let var = var_table.register_var(stmt::Type::list(stmt::Type::Unit));

        exec::ReadModifyWrite {
            input,
            output: Some(exec::Output {
                var,
                num_uses: node.num_uses.get(),
            }),
            read: self.read.clone(),
            write: self.write.clone(),
        }
    }
}

impl From<ReadModifyWrite> for mir::Node {
    fn from(value: ReadModifyWrite) -> Self {
        mir::Operation::ReadModifyWrite(Box::new(value)).into()
    }
}
