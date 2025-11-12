use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    exec,
    planner::{mir, VarTable},
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
        graph: &mir::Store,
        node: &mir::Node,
        var_table: &mut VarTable,
    ) -> exec::ReadModifyWrite {
        let input = self
            .inputs
            .iter()
            .map(|input| graph[input].var.get().unwrap())
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
