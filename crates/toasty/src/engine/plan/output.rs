use super::{eval, VarId};

#[derive(Debug, Clone)]
pub(crate) struct Output {
    /// Where to store the output
    pub var: VarId,

    /// How to project it before storing
    pub project: eval::Func,
}

#[derive(Debug, Clone)]
pub(crate) struct Output2 {
    /// Where to store the output
    pub var: VarId,

    /// Number of times the variable will be used
    pub num_uses: usize,
}
