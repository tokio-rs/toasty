use toasty_core::stmt;

use crate::engine::{exec, mir};

/// A constant list of values.
///
/// Used to inject static data into the operation graph, such as literal values
/// from the query or pre-computed results.
#[derive(Debug)]
pub(crate) struct Const {
    /// The constant rows.
    pub(crate) value: stmt::Value,

    /// The type of this constant.
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
            value: self.value.clone(),
        }
    }
}

impl From<Const> for mir::Node {
    fn from(value: Const) -> Self {
        mir::Operation::Const(value).into()
    }
}
