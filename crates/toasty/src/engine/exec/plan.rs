use crate::engine::exec::{Action, VarId, VarStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Atomicity {
    None,
    InteractiveTransaction,
    AtomicBatch,
}

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

    /// How database operations in this plan must be committed atomically.
    pub(crate) atomicity: Atomicity,
}
