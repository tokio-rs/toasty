use crate::engine::plan::VarId;

#[derive(Debug, Clone)]
pub(crate) struct Output2 {
    /// Where to store the output
    pub var: VarId,

    /// Number of times the variable will be used
    pub num_uses: usize,
}
