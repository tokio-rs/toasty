use crate::engine::{
    exec::{ExecPlan, VarStore},
    plan::ExecPlanner,
};

impl ExecPlanner<'_> {
    pub(super) fn plan_execution(mut self) -> ExecPlan {
        // Convert each node in execution order
        for node in self.logical_plan.operations() {
            let action = node.to_exec(self.logical_plan, &mut self.var_decls);
            self.actions.push(action);
        }

        let returning = self.logical_plan.completion().var.get();

        // A plan touching several statements is wrapped in a transaction for
        // one of two reasons: so its writes commit together, or so its reads
        // see one snapshot. Only the second is optional — a driver whose
        // backend has no transactions still needs its reads to run, whereas
        // letting its writes proceed unwrapped would drop atomicity silently.
        let db_ops = self.actions.iter().filter(|a| a.is_db_op()).count();
        let writes = self.actions.iter().any(|action| action.is_write());
        let needs_transaction =
            self.use_transactions && db_ops > 1 && (writes || self.snapshot_reads);

        ExecPlan {
            vars: VarStore::new(self.var_decls, self.schema),
            actions: self.actions,
            returning,
            needs_transaction,
        }
    }
}
