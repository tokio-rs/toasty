use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Executes a single-row upsert on a non-SQL database.
///
/// The planner emits this operation after lowering the conflict target and
/// verifying the requested upsert behavior against the driver's capabilities.
#[derive(Debug)]
pub(crate) struct Upsert {
    /// Nodes whose outputs are passed as arguments to the statement.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The lowered insert and conflict action.
    pub(crate) stmt: stmt::Insert,

    /// The return type of this operation.
    pub(crate) ty: stmt::Type,
}

impl Upsert {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Upsert {
        let input = self
            .inputs
            .iter()
            .map(|input| logical_plan[input].var.get().unwrap())
            .collect();

        let output = var_table.register_var(self.ty.clone());
        node.var.set(Some(output));

        exec::Upsert {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            stmt: self.stmt.clone(),
            ret: mir::row_field_types(&self.ty),
        }
    }
}

impl From<Upsert> for mir::Node {
    fn from(value: Upsert) -> Self {
        mir::Operation::Upsert(Box::new(value)).into()
    }
}
