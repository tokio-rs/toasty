mod materialize;

use crate::{
    engine::{
        exec::{ExecPlan, VarDecls, VarStore},
        mir, Engine,
    },
    Result,
};
use toasty_core::stmt;

#[derive(Debug)]
struct Planner<'a> {
    /// Handle to the schema & driver capabilities.
    engine: &'a Engine,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<ExecPlan> {
        let mut planner = Planner { engine: self };

        planner.plan_stmt_root(stmt)
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

impl<'a> Planner<'a> {
    /// Entry point to plan the root statement.
    fn plan_stmt_root(&mut self, stmt: stmt::Statement) -> Result<ExecPlan> {
        if let stmt::Statement::Insert(stmt) = &stmt {
            assert!(matches!(
                stmt.returning,
                Some(stmt::Returning::Model { .. })
            ));
        }

        let hir = self.engine.lower_stmt(stmt)?;

        // Build the logical plan
        let logical_plan = self.plan_statement(&hir);

        // Build the execution plan from the logical plan
        Ok(self.engine.build_exec_plan(logical_plan))
    }
}
