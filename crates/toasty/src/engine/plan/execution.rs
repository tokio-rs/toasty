use crate::engine::{
    effect::Effect,
    exec::{ExecPlan, PlanTransaction, VarStore},
    plan::ExecPlanner,
};

use toasty_core::driver::operation::IsolationLevel;

impl ExecPlanner<'_> {
    pub(super) fn plan_execution(mut self) -> ExecPlan {
        // Convert each node in execution order
        for node in self.logical_plan.operations() {
            let action = node.to_exec(self.logical_plan, &mut self.var_decls);
            self.actions.push(action);
        }

        let returning = self.logical_plan.completion().var.get();

        // A plan with a single database operation needs no transaction: one
        // statement already reads from one snapshot and commits atomically.
        let multi_op = self.actions.iter().filter(|a| a.is_db_op()).count() > 1;

        // Only SQL drivers take `Operation::Transaction`.
        let transaction = (self.capability.sql && multi_op).then(|| {
            if self.actions.iter().any(|a| a.effect() == Effect::Mutating) {
                // Mutating plans run at the database default. Raising
                // isolation for writes introduces serialization failures the
                // engine would have to retry.
                PlanTransaction {
                    isolation: None,
                    read_only: false,
                }
            } else {
                // Reads spanning several statements share one snapshot, so
                // an include cannot return relations that never coexisted
                // with the record they hang off. `read_only` makes a
                // misclassified plan error instead of silently writing.
                PlanTransaction {
                    isolation: self
                        .capability
                        .repeatable_read
                        .then_some(IsolationLevel::RepeatableRead),
                    read_only: true,
                }
            }
        });

        ExecPlan {
            vars: VarStore::new(self.var_decls, self.schema),
            actions: self.actions,
            returning,
            transaction,
        }
    }
}
