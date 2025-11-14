use toasty_core::stmt;

use crate::engine::{exec, mir};

#[derive(Debug)]
pub(crate) struct Const {
    pub(crate) value: Vec<stmt::Value>,
    pub(crate) ty: stmt::Type,
}

impl Const {
    pub(crate) fn to_exec(&self, node: &mir::Node, var_table: &mut exec::VarDecls) -> exec::SetVar {
        let var = var_table.register_var(node.ty().clone());
        node.var.set(Some(var));

        exec::SetVar {
            output: exec::Output {
                var,
                num_uses: node.num_uses.get(),
            },
            rows: self.value.clone(),
        }
    }
}

impl From<Const> for mir::Node {
    fn from(value: Const) -> Self {
        mir::Operation::Const(value).into()
    }
}
