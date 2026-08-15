use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

/// Passes another node's output through unchanged.
///
/// Fills a reserved node slot (see [`Store::reserve`](super::Store::reserve))
/// once planning knows which node actually produces the slot's value. Other
/// statements reference the reserved slot before its producer is planned; the
/// alias makes that reference concrete without rewriting their edges. At
/// execution the pass-through is a move between variable slots — rows are
/// never touched. A future copy-propagation pass may eliminate these.
#[derive(Debug)]
pub(crate) struct Alias {
    /// The node whose output is passed through.
    pub(crate) input: mir::NodeId,

    /// The output type (same as input).
    pub(crate) ty: stmt::Type,
}

impl Alias {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Alias {
        let input = logical_plan[self.input].var.get().unwrap();

        let var = var_table.register_var(self.ty.clone());
        node.var.set(Some(var));

        exec::Alias {
            input,
            output: exec::Output {
                var,
                num_uses: node.num_uses.get(),
            },
        }
    }
}

impl From<Alias> for mir::Node {
    fn from(value: Alias) -> Self {
        mir::Operation::Alias(value).into()
    }
}
