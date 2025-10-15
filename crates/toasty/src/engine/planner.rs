mod delete;
mod input;
mod insert;
mod kv;
mod lower;
mod ng;
mod output;
mod relation;
mod select;
mod subquery;
mod update;
mod verify;

mod var;
use var::VarTable;

use crate::{
    engine::{eval, plan, simplify, Engine, Plan},
    Result,
};
use toasty_core::{
    driver::Capability,
    schema::*,
    stmt::{self},
};

use std::collections::HashMap;

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

    /// In-progress write batch. This will be empty for read-only statements.
    write_actions: Vec<plan::WriteAction>,

    /// Variable to return as the result of the pipeline execution
    returning: Option<plan::VarId>,

    /// Tracks additional needed state to handle insertions.
    insertions: HashMap<app::ModelId, Insertion>,

    /// Planning a query can require walking relations to maintain data
    /// consistency. This field tracks the current relation edge being traversed
    /// so the planner doesn't walk it backwards.
    relations: Vec<app::FieldId>,
}

#[derive(Debug)]
struct Insertion {
    /// Insert plan entry
    action: usize,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<Plan> {
        let mut planner = Planner {
            engine: self,
            var_table: VarTable::default(),
            actions: vec![],
            write_actions: vec![],
            returning: None,
            insertions: HashMap::new(),
            relations: Vec::new(),
        };

        planner.plan_stmt_root(stmt)?;
        planner.build()
    }
}

impl<'a> Planner<'a> {
    fn schema(&self) -> &'a Schema {
        &self.engine.schema
    }

    fn capability(&self) -> &'a Capability {
        self.engine.capability()
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

        if let Some(output) = self.plan_stmt(stmt)? {
            self.returning = Some(output);
        }

        Ok(())
    }

    fn plan_stmt(&mut self, mut stmt: stmt::Statement) -> Result<Option<plan::VarId>> {
        self.simplify_stmt(&mut stmt);

        Ok(match stmt {
            stmt::Statement::Delete(stmt) => {
                self.plan_stmt_delete(stmt)?;
                None
            }
            stmt::Statement::Insert(stmt) => self.plan_stmt_insert(stmt)?,
            stmt::Statement::Query(stmt) => Some(self.plan_stmt_select(stmt)?),
            stmt::Statement::Update(stmt) => self.plan_stmt_update(stmt)?,
        })
    }

    fn build(mut self) -> Result<Plan> {
        match self.write_actions.len() {
            // Nothing to do here
            0 => {}
            1 => {
                let action = self.write_actions.drain(..).next().unwrap();
                self.push_action(action);
            }
            _ => {
                let action = plan::BatchWrite {
                    items: std::mem::take(&mut self.write_actions),
                };

                self.push_action(action);
            }
        }

        Ok(Plan {
            vars: exec::VarStore::new(self.var_table.into_vec()),
            pipeline: plan::Pipeline {
                actions: self.actions,
                returning: self.returning,
            },
        })
    }

    fn set_var(&mut self, value: Vec<stmt::Value>, ty: stmt::Type) -> plan::VarId {
        debug_assert!(ty.is_list());
        let var = self.var_table.register_var(ty);

        self.push_action(plan::SetVar { var, value });

        var
    }

    fn push_action(&mut self, action: impl Into<plan::Action>) {
        let action = action.into();
        self.verify_action(&action);
        self.actions.push(action);
    }

    fn push_write_action(&mut self, action: impl Into<plan::WriteAction>) {
        let action = action.into();
        self.verify_write_action(&action);
        self.write_actions.push(action);
    }

    pub(crate) fn take_const_var(&mut self, var: plan::VarId) -> Vec<stmt::Value> {
        let Some(action) = self.actions.pop() else {
            todo!()
        };
        let action = action.into_set_var();

        // The vars match
        assert_eq!(action.var, var);

        // TODO: release var slot!

        action.value
    }

    fn model(&self, id: impl Into<app::ModelId>) -> &'a app::Model {
        self.engine.schema.app.model(id)
    }

    fn simplify_stmt(&self, stmt: &mut stmt::Statement) {
        simplify::simplify_stmt(&self.engine.schema, stmt);

        // Make sure `via` associations is simplified
        debug_assert!(match stmt {
            stmt::Statement::Delete(stmt) => {
                match &stmt.from {
                    stmt::Source::Model(source) => source.via.is_none(),
                    stmt::Source::Table(_) => true,
                }
            }
            stmt::Statement::Insert(stmt) => {
                match &stmt.target {
                    stmt::InsertTarget::Scope(query) => match &query.body.as_select_unwrap().source
                    {
                        stmt::Source::Model(source) => source.via.is_none(),
                        stmt::Source::Table(_) => true,
                    },
                    _ => true,
                }
            }
            stmt::Statement::Query(stmt) => {
                match &stmt.body {
                    stmt::ExprSet::Select(select) => match &select.source {
                        stmt::Source::Model(source) => source.via.is_none(),
                        stmt::Source::Table(_) => true,
                    },
                    _ => true,
                }
            }
            _ => true,
        });
    }
}
