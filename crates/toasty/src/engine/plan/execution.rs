use indexmap::IndexSet;

use crate::engine::{
    exec::{self, Action, ExecPlan, VarStore},
    mir,
    plan::ExecPlanner,
};

impl ExecPlanner<'_> {
    pub(super) fn plan_execution(mut self) -> ExecPlan {
        // Group each maximal run of consecutive same-guard nodes into one
        // `If` block, emitted at the run's existing position. An unguarded
        // chain interleaved between two same-guard runs yields several `If`
        // blocks with the same condition; the guard rules guarantee an
        // interleaved unguarded node never consumes a guarded output, and the
        // else-arm variable classification below is per block.
        let logical_plan = self.logical_plan;
        let mut block: Vec<mir::NodeId> = vec![];
        let mut block_actions: Vec<Action> = vec![];
        let mut block_guard: Option<&mir::Cond> = None;

        for &node_id in logical_plan.execution_order() {
            let node = &logical_plan[node_id];
            let guard = node.guard.as_ref();

            let extends_block = match (guard, block_guard) {
                (Some(a), Some(b)) => a.groups_with(b),
                _ => false,
            };

            if block_guard.is_some() && !extends_block {
                let action = self.emit_if_block(
                    block_guard.take().unwrap(),
                    std::mem::take(&mut block),
                    std::mem::take(&mut block_actions),
                );
                self.actions.push(action);
            }

            let action = node.to_exec(logical_plan, &mut self.var_decls);

            if guard.is_some() {
                block_guard = guard;
                block.push(node_id);
                block_actions.push(action);
            } else {
                self.actions.push(action);
            }
        }

        if let Some(guard) = block_guard {
            let action = self.emit_if_block(guard, block, block_actions);
            self.actions.push(action);
        }

        let returning = self.logical_plan.completion().var.get();

        let needs_transaction = self.use_transactions
            && self.actions.iter().map(Action::db_op_count).sum::<usize>() > 1;

        ExecPlan {
            vars: VarStore::new(self.var_decls, self.schema),
            actions: self.actions,
            returning,
            needs_transaction,
        }
    }

    /// Wraps a run of same-guard actions in an `If`, deriving the skip
    /// bookkeeping from a static classification of the variables the `then`
    /// arm touches:
    ///
    /// - **External inputs** (produced outside, loaded inside): released on
    ///   skip, one entry per load the `then` arm would have performed,
    ///   keeping use counts exact on both paths.
    /// - **Escaping outputs** (produced inside, consumed outside): assigned
    ///   the empty value of the variable's type on skip, with the variable's
    ///   external use count, so outside consumers never see an unset slot.
    /// - **Internal variables** (produced and consumed inside): untouched —
    ///   on the skip path their slots are never created.
    fn emit_if_block(
        &self,
        guard: &mir::Cond,
        block: Vec<mir::NodeId>,
        block_actions: Vec<Action>,
    ) -> Action {
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
                    skipped_inputs.push(self.logical_plan[load].var.get().unwrap());
                }
            }

            // External use count: total uses minus the loads performed by
            // consumers inside the block.
            let in_block_loads = block
                .iter()
                .flat_map(|id| self.logical_plan[id].op.input_loads())
                .filter(|&load| load == node_id)
                .count();
            let external_uses = node.num_uses.get() - in_block_loads;

            if external_uses > 0 {
                empty_outputs.push((node.var.get().unwrap(), external_uses));
            }
        }

        let cond = match guard {
            mir::Cond::NonEmpty(node_id) => {
                exec::Cond::NonEmpty(self.logical_plan[node_id].var.get().unwrap())
            }
            mir::Cond::Expr { func, inputs } => exec::Cond::Expr {
                func: func.clone(),
                inputs: inputs
                    .iter()
                    .map(|node_id| self.logical_plan[node_id].var.get().unwrap())
                    .collect(),
            },
        };

        exec::If {
            cond,
            then: block_actions,
            skipped_inputs,
            empty_outputs,
        }
        .into()
    }
}
