use std::collections::HashSet;

use crate::engine::{
    exec::{Block, ExecPlan, Terminator, VarStore},
    mir::{self, Operation},
    plan::ExecPlanner,
};

impl ExecPlanner<'_> {
    pub(super) fn plan_execution(mut self) -> ExecPlan {
        // Pre-scan: collect then_node IDs from IfNonEmpty operations into a
        // "deferred" set so they are skipped in the normal linear flow. Also
        // bump the guard's num_uses by 1 for the runtime emptiness check.
        let mut deferred: HashSet<mir::NodeId> = HashSet::new();
        for (_, node) in self.logical_plan.operations_with_ids() {
            if let Operation::IfNonEmpty(m) = &node.op {
                deferred.insert(m.then_node);

                let guard_node = &self.logical_plan[m.guard];
                guard_node.num_uses.set(guard_node.num_uses.get() + 1);
            }
        }

        // Build blocks. Start with a single entry block.
        let mut blocks: Vec<Block> = vec![Block {
            actions: vec![],
            terminator: Terminator::Return, // placeholder
        }];
        let mut current_block: usize = 0;

        for (node_id, node) in self.logical_plan.operations_with_ids() {
            // Skip deferred nodes — emitted inside then-blocks below.
            if deferred.contains(&node_id) {
                continue;
            }

            if let Operation::IfNonEmpty(m) = &node.op {
                let guard_node = &self.logical_plan[m.guard];
                let guard_var = guard_node
                    .var
                    .get()
                    .expect("guard node var not yet assigned");

                // Convert the deferred then_node to an exec action.
                let then_node = &self.logical_plan[m.then_node];
                let then_action = then_node.to_exec(self.logical_plan, &mut self.var_decls);

                // Register a var for the IfNonEmpty node itself.
                let if_var = self.var_decls.register_var(m.ty.clone());
                node.var.set(Some(if_var));

                // Continuation block.
                let continuation_idx = blocks.len();
                blocks.push(Block {
                    actions: vec![],
                    terminator: Terminator::Return, // placeholder
                });

                // Then-block.
                let then_idx = blocks.len();
                blocks.push(Block {
                    actions: vec![then_action],
                    terminator: Terminator::Goto(continuation_idx),
                });

                // Else-block (empty).
                let else_idx = blocks.len();
                blocks.push(Block {
                    actions: vec![],
                    terminator: Terminator::Goto(continuation_idx),
                });

                blocks[current_block].terminator = Terminator::IfNonEmpty {
                    var: guard_var,
                    then_block: then_idx,
                    else_block: else_idx,
                };

                current_block = continuation_idx;
            } else {
                let action = node.to_exec(self.logical_plan, &mut self.var_decls);
                blocks[current_block].actions.push(action);
            }
        }

        blocks[current_block].terminator = Terminator::Return;

        let returning = self.logical_plan.completion().var.get();

        let needs_transaction = self.use_transactions
            && blocks
                .iter()
                .flat_map(|b| b.actions.iter())
                .filter(|a| a.is_db_op())
                .count()
                > 1;

        ExecPlan {
            vars: VarStore::new(self.var_decls),
            blocks,
            entry: 0,
            returning,
            needs_transaction,
        }
    }
}
