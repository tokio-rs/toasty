use crate::engine::exec::{Action, VarId, VarStore};

use toasty_core::driver::operation::IsolationLevel;

/// How the executor starts the transaction wrapping a plan.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlanTransaction {
    /// Minimum isolation level, or `None` for the database default.
    pub(crate) isolation: Option<IsolationLevel>,

    /// Whether the transaction is read-only.
    pub(crate) read_only: bool,
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

    /// When set, the executor wraps the entire plan in a transaction.
    pub(crate) transaction: Option<PlanTransaction>,
}
