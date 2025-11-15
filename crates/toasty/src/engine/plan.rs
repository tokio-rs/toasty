mod nested_merge;
mod statement;

use crate::{
    engine::{
        exec::{ExecPlan, VarDecls, VarStore},
        mir, Engine, HirStatement,
    },
    Result,
};

#[derive(Debug)]
struct PlanStatement<'a> {
    engine: &'a Engine,

    /// Root statement and all nested statements.
    hir: &'a HirStatement,

    /// Graph of operations needed to execute the statement
    mir: mir::Store,
}

impl Engine {
    pub(super) fn plan_statement(&self, hir: HirStatement) -> Result<ExecPlan> {
        // Build the logical plan
        let logical_plan = PlanStatement {
            engine: self,
            hir: &hir,
            mir: mir::Store::new(),
        }
        .build_logical_plan();

        // Build the execution plan from the logical plan
        Ok(self.plan_execution(logical_plan))
    }

    pub(super) fn plan_execution(&self, logical_plan: mir::LogicalPlan) -> ExecPlan {
        let mut var_table = VarDecls::default();
        let mut actions = Vec::new();

        // Convert each node in execution order
        for node in logical_plan.operations() {
            let action = node.to_exec(&logical_plan, &mut var_table);
            actions.push(action);
        }

        let returning = logical_plan.completion().var.get();

        ExecPlan {
            vars: VarStore::new(var_table.into_vec()),
            actions,
            returning,
        }
    }
}

impl PlanStatement<'_> {}
