use super::*;

#[derive(Debug)]
pub(crate) struct Pipeline {
    /// Steps in the pipeline
    pub(crate) actions: Vec<Action>,

    /// Which record stream slot does the pipeline return
    ///
    /// When `None`, nothing is returned
    pub(crate) returning: Option<VarId>,
}
