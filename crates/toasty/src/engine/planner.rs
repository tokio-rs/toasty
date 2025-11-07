mod ng;
mod verify;

mod var;
use var::VarTable;

use crate::{
    engine::{plan, Engine, Plan},
    Result,
};
use toasty_core::{
    stmt::{self},
    Schema,
};

use super::exec;

#[derive(Debug)]
struct Planner<'a> {
    /// Handle to the schema & driver capabilities.
    engine: &'a Engine,

    /// Table of record stream slots. Used to figure out where to store outputs
    /// of actions.
    var_table: VarTable,

    /// Actions that will end up in the pipeline.
    actions: Vec<plan::Action>,

    /// Variable to return as the result of the pipeline execution
    returning: Option<plan::VarId>,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<Plan> {
        let mut planner = Planner {
            engine: self,
            var_table: VarTable::default(),
            actions: vec![],
            returning: None,
        };

        planner.plan_stmt_root(stmt)?;
        planner.build()
    }
}

impl<'a> Planner<'a> {
    fn schema(&self) -> &'a Schema {
        &self.engine.schema
    }

    /// Entry point to plan the root statement.
    fn plan_stmt_root(&mut self, stmt: stmt::Statement) -> Result<()> {
        if let stmt::Statement::Insert(stmt) = &stmt {
            // TODO: this isn't always true. The assert is there to help
            // debug old code.
            assert!(matches!(
                stmt.returning,
                Some(stmt::Returning::Model { .. })
            ));
        }

        let output = self.plan_v2_stmt(stmt)?;

        if let Some(output) = output {
            self.returning = Some(output);
        }

        Ok(())
    }

    fn build(self) -> Result<Plan> {
        Ok(Plan {
            vars: exec::VarStore::new(self.var_table.into_vec()),
            pipeline: plan::Pipeline {
                actions: self.actions,
                returning: self.returning,
            },
        })
    }

    fn push_action(&mut self, action: impl Into<plan::Action>) {
        let action = action.into();
        self.verify_action(&action);
        self.actions.push(action);
    }
}
