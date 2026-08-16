use indexmap::IndexSet;

use crate::engine::{eval, mir};

/// Evaluates `body` once over the whole outputs of `inputs`.
///
/// `arg(i)` in the body is `inputs[i]`'s complete output. For per-row
/// evaluation over one input's rows, use [`MapOver`](mir::MapOver) instead.
#[derive(Debug)]
pub(crate) struct Compute {
    /// The nodes whose outputs the body reads.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The function to evaluate: `arg(i)` = `inputs[i]`.
    pub(crate) body: eval::Func,
}

impl From<Compute> for mir::Node {
    fn from(value: Compute) -> Self {
        mir::Operation::Compute(value).into()
    }
}
