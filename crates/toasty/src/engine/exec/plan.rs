use crate::engine::exec::{Action, VarId, VarStore};

#[derive(Debug)]
pub(crate) struct ExecPlan {
    /// Arguments seeding the plan
    pub(crate) vars: VarStore,

    /// Steps in the pipeline
    pub(crate) actions: Vec<Action>,

    /// Which record stream slot does the pipeline return
    ///
    /// When `None`, nothing is returned
    pub(crate) returning: Option<VarId>,
}
