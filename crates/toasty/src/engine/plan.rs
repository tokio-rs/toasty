mod materialize;

use crate::{
    engine::{
        exec::{ExecPlan, VarDecls, VarStore},
        mir, Engine,
    },
    Result,
};
use toasty_core::stmt;

impl Engine {
    pub(crate) fn plan_statement(&self, stmt: stmt::Statement) -> Result<ExecPlan> {
        if let stmt::Statement::Insert(stmt) = &stmt {
            assert!(matches!(
                stmt.returning,
                Some(stmt::Returning::Model { .. })
            ));
        }

        let hir = self.lower_stmt(stmt)?;

        // Build the logical plan
        let logical_plan = self.plan_hir_statement(&hir);

        // Build the execution plan from the logical plan
        Ok(self.build_exec_plan(logical_plan))
    }

    pub(crate) fn build_exec_plan(&self, logical_plan: mir::LogicalPlan) -> ExecPlan {
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
