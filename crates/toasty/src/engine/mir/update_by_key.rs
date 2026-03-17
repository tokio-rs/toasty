use toasty_core::{schema::db::TableId, stmt};

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Updates records by primary key.
///
/// Used with NoSQL drivers to update records given a list of primary key values.
///
/// Keys are always specified as an input node, whether a [`Const`] or the
/// output of a dependent operation.
#[derive(Debug)]
pub(crate) struct UpdateByKey {
    /// The node producing the list of primary keys to update.
    pub(crate) input: mir::NodeId,

    /// Nodes whose outputs are passed as arguments for substitution in
    /// assignments and filter expressions.
    pub(crate) arg_inputs: Vec<mir::NodeId>,

    /// The table to update records in.
    pub(crate) table: TableId,

    /// The field assignments to apply.
    pub(crate) assignments: stmt::Assignments,

    /// Optional additional filter applied before update.
    pub(crate) filter: Option<stmt::Expr>,

    /// Optional condition for optimistic locking.
    pub(crate) condition: Option<stmt::Expr>,

    /// The return type.
    pub(crate) ty: stmt::Type,

    /// Optional guard node. If the guard's output is empty, this operation
    /// produces empty results instead of executing.
    pub(crate) guard: Option<mir::NodeId>,
}

impl UpdateByKey {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::UpdateByKey {
        let input = logical_plan[self.input].var.get().unwrap();
        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        let guard = self
            .guard
            .map(|guard_id| logical_plan[guard_id].var.get().unwrap());

        let arg_inputs = self
            .arg_inputs
            .iter()
            .map(|node_id| logical_plan[node_id].var.get().unwrap())
            .collect();

        exec::UpdateByKey {
            input,
            arg_inputs,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            assignments: self.assignments.clone(),
            filter: self.filter.clone(),
            condition: self.condition.clone(),
            returning: !self.ty.is_unit(),
            guard,
        }
    }
}

impl From<UpdateByKey> for mir::Node {
    fn from(value: UpdateByKey) -> Self {
        mir::Operation::UpdateByKey(value).into()
    }
}
