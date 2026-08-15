use indexmap::IndexSet;
use toasty_core::stmt;

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
        let mut block: Vec<mir::NodeId> = vec![];
        let mut block_actions: Vec<Action> = vec![];
        let mut block_guard: Option<mir::NodeId> = None;

        for &node_id in self.logical_plan.execution_order() {
            let node = &self.logical_plan[node_id];
            let guard = node.guard.get();

            if block_guard.is_some() && guard != block_guard {
                let action = self.emit_if_block(
                    block_guard.take().unwrap(),
                    std::mem::take(&mut block),
                    std::mem::take(&mut block_actions),
                );
                self.actions.push(action);
            }

            let action = node.to_exec(self.logical_plan, &mut self.var_decls);

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

    /// Wraps a run of same-guard actions in an `If`, generating the else arm
    /// from a static classification of the variables the `then` arm touches:
    ///
    /// - **Escaping outputs** (produced inside, consumed outside): a `SetVar`
    ///   assigning the empty value of the variable's type, with the
    ///   variable's external use count, so outside consumers never see an
    ///   unset slot.
    /// - **External inputs** (produced outside, loaded inside): one release
    ///   per load the `then` arm would have performed, keeping use counts
    ///   exact on both paths.
    /// - **Internal variables** (produced and consumed inside): untouched —
    ///   on the else path their slots are never created.
    fn emit_if_block(
        &self,
        guard: mir::NodeId,
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
        let mut r#else: Vec<Action> = vec![];

        for &node_id in &block {
            let node = &self.logical_plan[node_id];

            // Loads the `then` arm performs on variables produced outside the
            // block are released, with multiplicity.
            for load in node.op.input_loads() {
                if !in_block.contains(&load) {
                    r#else.push(
                        exec::Release {
                            var: self.logical_plan[load].var.get().unwrap(),
                        }
                        .into(),
                    );
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
                let value = match node.ty() {
                    stmt::Type::List(_) => stmt::Value::List(vec![]),
                    _ => stmt::Value::Null,
                };

                r#else.push(
                    exec::SetVar {
                        value,
                        output: exec::Output {
                            var: node.var.get().unwrap(),
                            num_uses: external_uses,
                        },
                    }
                    .into(),
                );
            }
        }

        exec::If {
            cond: exec::Cond::NonEmpty(self.logical_plan[guard].var.get().unwrap()),
            then: block_actions,
            r#else,
        }
        .into()
    }
}
