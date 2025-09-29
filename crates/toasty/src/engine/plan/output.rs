use toasty_core::stmt::Type;

use super::{eval, VarId};

#[derive(Debug, Clone)]
pub(crate) struct Output {
    /// The Toasty-level type returned by the database
    pub ty: Vec<Type>,

    /// What to do with the output. This may end up being fanned out to multiple
    /// variables.
    pub targets: Vec<OutputTarget>,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputTarget {
    /// Where to store the output
    pub var: VarId,

    /// How to project it before storing
    pub project: eval::Func,
}

impl Output {
    pub fn single_target(var: VarId, project: eval::Func) -> Output {
        Output {
            ty: todo!(),
            targets: vec![OutputTarget { var, project }],
        }
    }
}
