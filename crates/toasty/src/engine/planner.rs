mod materialize;

mod lower;

mod var;
pub(super) use var::VarTable;

use crate::{
    engine::{
        exec::{ExecPlan, VarId},
        Engine,
    },
    Result,
};
use toasty_core::{stmt, Schema};

use super::{exec, hir};

#[derive(Debug)]
struct Planner<'a> {
    /// Handle to the schema & driver capabilities.
    engine: &'a Engine,

    /// Stores decomposed statement info
    store: hir::Store,

    /// Graph of materialization steps to execute the original statement being
    /// planned.
    graph: super::mir::Store,

    /// Table of record stream slots. Used to figure out where to store outputs
    /// of actions.
    var_table: VarTable,

    /// Actions that will end up in the pipeline.
    actions: Vec<exec::Action>,

    /// Variable to return as the result of the pipeline execution
    returning: Option<exec::VarId>,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<ExecPlan> {
        let mut planner = Planner {
            engine: self,
            store: super::hir::Store::new(),
            graph: super::mir::Store::new(),
            var_table: VarTable::default(),
            actions: vec![],
            returning: None,
        };

        planner.plan_stmt_root(stmt)?;
        planner.build()
    }
}

impl<'a> Planner<'a> {
    pub(crate) fn schema(&self) -> &'a Schema {
        &self.engine.schema
    }

    /// Entry point to plan the root statement.
    fn plan_stmt_root(&mut self, stmt: stmt::Statement) -> Result<()> {
        if let stmt::Statement::Insert(stmt) = &stmt {
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

    fn plan_v2_stmt(&mut self, stmt: stmt::Statement) -> Result<Option<VarId>> {
        self.lower_stmt(stmt)?;

        // Build the execution plan...
        self.plan_materializations();

        let mid = self.store.root().output.get().unwrap();
        let node = &self.graph[mid];
        node.num_uses.set(node.num_uses.get() + 1);

        // Build the execution plan
        for node_id in &self.graph.execution_order {
            let node = &self.graph[node_id];
            let action = node.to_exec(&self.graph, &mut self.var_table);
            self.actions.push(action);
        }

        let mid = self.store.root().output.get().unwrap();
        Ok(self.graph[mid].var.get())
    }

    fn build(self) -> Result<ExecPlan> {
        Ok(ExecPlan {
            vars: exec::VarStore::new(self.var_table.into_vec()),
            actions: self.actions,
            returning: self.returning,
        })
    }
}
