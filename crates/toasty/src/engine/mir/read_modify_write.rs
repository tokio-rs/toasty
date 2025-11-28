use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Performs an optimistic read-modify-write operation.
///
/// Used for conditional updates where the write only succeeds if the values
/// read have not been modified since reading. This is a fallback for databases
/// that do not support conditional updates in a single statement (e.g., SQLite,
/// MySQL without CTE support).
#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Nodes providing input arguments for the statements.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The read query that fetches current values.
    pub(crate) read: stmt::Query,

    /// The write statement to execute if the condition holds.
    pub(crate) write: stmt::Statement,

    /// The return type.
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
