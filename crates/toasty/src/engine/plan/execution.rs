use indexmap::IndexSet;

use crate::engine::{
    exec::{self, Step},
    mir,
    plan::ExecPlanner,
};

impl ExecPlanner<'_> {
    /// Converts the logical plan's execution order into the exec program's
    /// step sequence, returning the steps and whether the plan needs to be
    /// wrapped in a transaction.
    pub(super) fn plan_execution(mut self) -> (Vec<Step>, bool) {
        // Group each maximal run of consecutive same-guard nodes into one
        // `If` block, emitted at the run's existing position. An unguarded
        // chain interleaved between two same-guard runs yields several `If`
        // blocks with the same condition; the guard rules guarantee an
        // interleaved unguarded node never consumes a guarded output, and the
        // skip-path variable classification below is per block.
        let logical_plan = self.logical_plan;
        let mut block: Vec<mir::NodeId> = vec![];
        let mut block_guard: Option<mir::NodeId> = None;

        for &node_id in logical_plan.execution_order() {
            let node = &logical_plan[node_id];
            let guard = node.guard;

            let extends_block = guard.is_some() && guard == block_guard;

            if block_guard.is_some() && !extends_block {
                block_guard = None;
                let step = self.emit_if_block(std::mem::take(&mut block));
                self.steps.push(step);
            }

            if guard.is_some() {
                block_guard = guard;
                block.push(node_id);
            } else {
                self.steps.push(Step::Run(node_id));
            }
        }

        if !block.is_empty() {
            let step = self.emit_if_block(block);
            self.steps.push(step);
        }

        let needs_transaction = self.needs_transaction();

        (self.steps, needs_transaction)
    }

    fn needs_transaction(&self) -> bool {
        self.use_transactions
            && self
                .steps
                .iter()
                .map(|step| step.db_op_count(self.logical_plan))
                .sum::<usize>()
                > 1
    }

    /// Wraps a run of same-guard nodes in an `If`, deriving the skip
    /// bookkeeping from a static classification of the variables the `then`
    /// arm touches:
    ///
    /// - **External inputs** (produced outside, loaded inside): released on
    ///   skip, one entry per load the `then` arm would have performed,
    ///   keeping use counts exact on both paths.
    /// - **Escaping outputs** (produced inside, consumed outside): assigned
    ///   the empty value of the node's type on skip, with the node's
    ///   external use count, so outside consumers never see an unset slot.
    /// - **Internal variables** (produced and consumed inside): untouched —
    ///   on the skip path their slots are never created.
    fn emit_if_block(&self, block: Vec<mir::NodeId>) -> Step {
        debug_assert!(
            block
                .iter()
                .all(|id| !self.logical_plan[id].op.is_effectful()),
            "effectful node inside an `If` arm"
        );

        let in_block: IndexSet<mir::NodeId> = block.iter().copied().collect();
        let mut skipped_inputs = vec![];
        let mut empty_outputs = vec![];

        for &node_id in &block {
            let node = &self.logical_plan[node_id];

            // Loads the `then` arm performs on variables produced outside the
            // block are released, with multiplicity.
            for load in node.op.input_loads() {
                if !in_block.contains(&load) {
                    skipped_inputs.push(load);
                }
            }

            // External use count: total uses minus the loads performed by
            // consumers inside the block.
            let in_block_loads = block
                .iter()
                .flat_map(|id| self.logical_plan[id].op.input_loads())
                .filter(|&load| load == node_id)
                .count();
            let external_uses = self.logical_plan.num_uses(node_id) - in_block_loads;

            if external_uses > 0 {
                empty_outputs.push((node_id, external_uses));
            }
        }

        Step::If(exec::If {
            then: block,
            skipped_inputs,
            empty_outputs,
        })
    }
}
