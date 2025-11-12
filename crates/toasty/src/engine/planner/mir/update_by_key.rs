use toasty_core::{schema::db::TableId, stmt};

use crate::engine::{
    exec,
    planner::{mir, VarTable},
};

#[derive(Debug)]
pub(crate) struct UpdateByKey {
    pub(crate) input: mir::NodeId,

    pub(crate) table: TableId,

    pub(crate) assignments: stmt::Assignments,

    pub(crate) filter: Option<stmt::Expr>,

    pub(crate) condition: Option<stmt::Expr>,

    pub(crate) ty: stmt::Type,
}

impl UpdateByKey {
    pub(crate) fn to_exec(
        &self,
        graph: &mir::MaterializeGraph,
        node: &mir::Node,
        var_table: &mut VarTable,
    ) -> exec::UpdateByKey {
        let input = graph.var_id(self.input);
        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        exec::UpdateByKey {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            assignments: self.assignments.clone(),
            filter: self.filter.clone(),
            condition: self.condition.clone(),
            returning: !self.ty.is_unit(),
        }
    }
}

impl From<UpdateByKey> for mir::Node {
    fn from(value: UpdateByKey) -> Self {
        mir::Operation::UpdateByKey(value).into()
    }
}
