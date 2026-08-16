use crate::{
    Result,
    engine::{exec::Exec, mir, mir::LogicalPlan},
};

/// A conditionally executed block of nodes — the exec program's first
/// control-flow construct.
///
/// The `then` arm holds pure nodes whose outputs are only consumed when the
/// condition holds; mutations never appear in it. Every node in the arm
/// carries the same guard, which is the block's condition. Skipping the arm
/// is pure bookkeeping, declared as data: `skipped_inputs` and
/// `empty_outputs` keep variable slots consistent so consumers outside the
/// block never see an unset slot and use counts stay exact on both paths.
#[derive(Debug)]
pub(crate) struct If {
    /// Nodes run when the condition holds. Non-empty; the condition is the
    /// nodes' shared guard.
    pub(crate) then: Vec<mir::NodeId>,

    /// Nodes produced outside the block whose outputs the `then` arm would
    /// have loaded — one entry per declined load. Released when the arm is
    /// skipped.
    pub(crate) skipped_inputs: Vec<mir::NodeId>,

    /// The `then` arm's escaping outputs, each with its external use count.
    /// When the arm is skipped, each is assigned the empty value of its
    /// declared type.
    pub(crate) empty_outputs: Vec<(mir::NodeId, usize)>,
}

impl Exec<'_> {
    pub(super) async fn action_if(
        &mut self,
        logical_plan: &LogicalPlan,
        action: &If,
    ) -> Result<()> {
        let cond = logical_plan[action.then[0]]
            .guard
            .expect("`If` block nodes carry the block's guard");

        // A non-consuming peek: buffers the condition variable's stream in
        // place, leaving its use count untouched.
        let pass = self.vars.peek_non_empty(cond).await?;

        if pass {
            for &node_id in &action.then {
                self.exec_node(logical_plan, node_id).await?;
            }
        } else {
            for &node_id in &action.skipped_inputs {
                self.vars.release(node_id);
            }
            for &(node_id, num_uses) in &action.empty_outputs {
                self.vars
                    .store_empty(node_id, logical_plan[node_id].ty(), num_uses);
            }
        }

        Ok(())
    }
}
