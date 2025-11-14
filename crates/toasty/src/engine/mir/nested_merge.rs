use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{exec, mir};

#[derive(Debug)]
pub(crate) struct NestedMerge {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The root nested merge level
    pub(crate) root: exec::NestedLevel,
}

impl NestedMerge {
    pub(crate) fn to_exec(
        &self,
        graph: &mir::Store,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::NestedMerge {
        let mut input_vars = vec![];

        for input in &self.inputs {
            let var = graph[input].var.get().unwrap();
            input_vars.push(var);
        }

        let output = var_table.register_var(stmt::Type::list(self.root.projection.ret.clone()));
        node.var.set(Some(output));

        exec::NestedMerge {
            inputs: input_vars,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            root: self.root.clone(),
        }
    }
}

impl From<NestedMerge> for mir::Node {
    fn from(value: NestedMerge) -> Self {
        mir::Operation::NestedMerge(value).into()
    }
}
