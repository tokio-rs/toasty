use super::*;

#[derive(Debug, Clone)]
pub(crate) struct Output {
    /// Where to store the output
    pub var: VarId,

    /// How to project it before storing
    pub project: eval::Func,
}
