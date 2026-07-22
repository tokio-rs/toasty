use crate::engine::{
    effect::Effect,
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

        let needs_transaction = self.use_transactions
            && self.actions.iter().filter(|a| a.is_db_op()).count() > 1
            && self.actions.iter().any(|a| a.effect() == Effect::Mutating);

        ExecPlan {
            vars: VarStore::new(self.var_decls, self.schema),
            actions: self.actions,
            returning,
            needs_transaction,
        }
    }
}
