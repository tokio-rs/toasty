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

        ExecPlan {
            vars: VarStore::new(self.var_decls),
            actions: self.actions,
            returning,
        }
    }
}
