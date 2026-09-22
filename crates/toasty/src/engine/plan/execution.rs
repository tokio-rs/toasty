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
        self.plan_steps();

        let needs_transaction = self.needs_transaction();

        (self.steps, needs_transaction)
    }

    fn plan_steps(&mut self) {
        // Group each maximal run of consecutive same-guard nodes into one
        // `If` block, emitted at the run's existing position. An unguarded
        // chain interleaved between two same-guard runs yields several `If`
        // blocks with the same condition; the guard rules guarantee an
        // interleaved unguarded node never consumes a guarded output, and the
        // skip-path variable classification below is per block.
        let logical_plan = self.logical_plan;
        let mut if_block = vec![];

        for &node_id in logical_plan.execution_order() {
            self.plan_node(node_id, &mut if_block);
        }

        self.flush_if_block(&mut if_block);
    }

    fn plan_node(&mut self, node_id: mir::NodeId, if_block: &mut Vec<mir::NodeId>) {
        let guard = self.logical_plan[node_id].guard;

        if let Some(&first_node_id) = if_block.first()
            && guard != self.logical_plan[first_node_id].guard
        {
            self.flush_if_block(if_block);
        }

        if guard.is_some() {
            if_block.push(node_id);
        } else {
            self.steps.push(Step::Run(node_id));
        }
    }

    fn flush_if_block(&mut self, block: &mut Vec<mir::NodeId>) {
        if block.is_empty() {
            return;
        }

        let step = self.emit_if_block(std::mem::take(block));
        self.steps.push(step);
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
    /// - **Escaping outputs** (produced inside, consumed outside): assigned a
    ///   non-loadable slot on skip with the node's external use count, so
    ///   outside consumers can release the skipped output.
    /// - **Internal variables** (produced and consumed inside): untouched —
    ///   on the skip path their slots are never created.
    fn emit_if_block(&self, block: Vec<mir::NodeId>) -> Step {
        debug_assert!(
            block
                .iter()
                .all(|id| !self.logical_plan[id].op.is_effectful()),
            "effectful node inside an `If` arm"
        );

        let skipped_inputs = self.collect_skipped_inputs(&block);
        let skipped_outputs = self.collect_skipped_outputs(&block);

        Step::If(exec::If {
            then: block,
            skipped_inputs,
            skipped_outputs,
        })
    }

    /// Collects loads from variables produced outside the block. The skip arm
    /// releases each load that the `then` arm would have performed.
    fn collect_skipped_inputs(&self, block: &[mir::NodeId]) -> Vec<mir::NodeId> {
        let in_block: IndexSet<mir::NodeId> = block.iter().copied().collect();

        block
            .iter()
            .flat_map(|&node_id| self.logical_plan[node_id].op.input_loads())
            .filter(|load| !in_block.contains(load))
            .collect()
    }

    /// Collects variables produced inside the block and consumed outside it.
    fn collect_skipped_outputs(&self, block: &[mir::NodeId]) -> Vec<(mir::NodeId, usize)> {
        block
            .iter()
            .filter_map(|&node_id| {
                let external_uses = self.external_use_count(block, node_id);
                (external_uses > 0).then_some((node_id, external_uses))
            })
            .collect()
    }

    fn external_use_count(&self, block: &[mir::NodeId], node_id: mir::NodeId) -> usize {
        let in_block_loads = block
            .iter()
            .flat_map(|&consumer_id| self.logical_plan[consumer_id].op.input_loads())
            .filter(|&load| load == node_id)
            .count();

        self.logical_plan[node_id].num_uses - in_block_loads
    }
}
