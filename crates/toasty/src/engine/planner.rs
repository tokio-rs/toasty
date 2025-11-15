mod materialize;

use crate::{
    engine::{
        exec::{ExecPlan, VarDecls, VarStore},
        mir, Engine,
    },
    Result,
};
use toasty_core::stmt;

use super::hir;

#[derive(Debug)]
struct Planner<'a> {
    /// Handle to the schema & driver capabilities.
    engine: &'a Engine,

    /// Stores decomposed statement info
    store: hir::Store,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<ExecPlan> {
        let mut planner = Planner {
            engine: self,
            store: hir::Store::new(),
        };

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

        self.plan_v2_stmt(stmt)
    }

    fn plan_v2_stmt(&mut self, stmt: stmt::Statement) -> Result<ExecPlan> {
        let hir_stmt = self.engine.lower_stmt(stmt)?;
        self.store = hir_stmt.into_store();

        // Build the logical plan
        let logical_plan = self.plan_statement();

        // Build the execution plan from the logical plan
        Ok(self.engine.build_exec_plan(logical_plan))
    }
}
