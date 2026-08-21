use crate::engine::{
    exec::{If, VarStore},
    mir,
};

/// The executable form of a query: the MIR operation graph plus the step
/// sequence that runs it. Steps reference operations by [`mir::NodeId`],
/// which doubles as the runtime variable slot for the operation's output.
#[derive(Debug)]
pub(crate) struct ExecPlan {
    /// The operation graph the steps execute.
    pub(crate) plan: mir::LogicalPlan,

    /// Runtime storage for node outputs.
    pub(crate) vars: VarStore,

    /// Steps in the pipeline.
    pub(crate) steps: Vec<Step>,

    /// The node whose output the pipeline returns.
    pub(crate) returning: mir::NodeId,

    /// When true, the executor wraps the entire plan in a transaction.
    pub(crate) needs_transaction: bool,
}

/// A single step of the exec program.
#[derive(Debug)]
pub(crate) enum Step {
    /// Execute one node's operation and store its output.
    Run(mir::NodeId),

    /// Conditionally execute a block of pure nodes.
    If(If),
}

impl Step {
    /// Returns the step's name for logging.
    pub(crate) fn name(&self, plan: &mir::LogicalPlan) -> &'static str {
        match self {
            Step::Run(node_id) => plan[node_id].op.name(),
            Step::If(_) => "if",
        }
    }

    /// Returns the number of database operations this step issues, counting
    /// into `If` arms. The count is static: a skipped `If` arm can leave a
    /// transaction wrapping a single executed operation, which is harmless.
    pub(crate) fn db_op_count(&self, plan: &mir::LogicalPlan) -> usize {
        match self {
            Step::Run(node_id) => plan[node_id].op.is_db_op() as usize,
            Step::If(action) => action
                .then
                .iter()
                .filter(|node_id| plan[*node_id].op.is_db_op())
                .count(),
        }
    }
}
