mod delete;
mod index;
mod input;
mod insert;
mod key;
mod link;
mod lower;
mod output;
mod relation;
mod select;
mod simplify;
mod subquery;
mod unlink;
mod update;
mod verify;

mod var;
use var::VarTable;

use crate::{
    driver::capability::{self, Capability},
    engine::{plan, Plan},
};
use toasty_core::{
    eval,
    schema::*,
    stmt::{self, Visit, VisitMut},
};

use std::collections::HashMap;

use super::exec;

#[derive(Debug)]
struct Planner<'a> {
    /// Database schema
    schema: &'a Schema,

    /// Database capabilities
    capability: &'a Capability,

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
    insertions: HashMap<ModelId, Insertion>,

    /// Each subquery is planned individually and the output variable is tracked
    /// here.
    ///
    /// TODO: make key a new-type?
    subqueries: HashMap<usize, plan::VarId>,

    /// Planning a query can require walking relations to maintain data
    /// consistency. This field tracks the current relation edge being traversed
    /// so the planner doesn't walk it backwards.
    relations: Vec<FieldId>,
}

#[derive(Debug)]
struct Insertion {
    /// Insert plan entry
    action: usize,
}

pub(crate) fn apply(capability: &Capability, schema: &Schema, stmt: stmt::Statement) -> Plan {
    let mut planner = Planner {
        capability,
        schema,
        var_table: VarTable::default(),
        actions: vec![],
        write_actions: vec![],
        returning: None,
        insertions: HashMap::new(),
        subqueries: HashMap::new(),
        relations: Vec::new(),
    };

    planner.plan_stmt(stmt);
    planner.build()
}

impl<'a> Planner<'a> {
    /// Entry point to plan the root statement.
    fn plan_stmt(&mut self, stmt: stmt::Statement) {
        match stmt {
            stmt::Statement::Delete(stmt) => self.plan_delete(stmt),
            stmt::Statement::Link(stmt) => self.plan_link(stmt),
            stmt::Statement::Insert(stmt) => {
                // TODO: this isn't always true. The assert is there to help
                // debug old code.
                assert_eq!(stmt.returning, Some(stmt::Returning::Star));

                let output_var = self.plan_insert(stmt);
                assert!(output_var.is_some());
                self.returning = output_var;
            }
            stmt::Statement::Query(stmt) => {
                let output = self.plan_select(stmt);
                self.returning = Some(output);
            }
            stmt::Statement::Unlink(stmt) => self.plan_unlink(stmt),
            stmt::Statement::Update(stmt) => {
                if let Some(output) = self.plan_update(stmt) {
                    self.returning = Some(output);
                }
            }
        }
    }

    fn build(mut self) -> Plan {
        let vars = exec::VarStore::new();

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

        Plan {
            vars,
            pipeline: plan::Pipeline {
                actions: self.actions,
                returning: self.returning,
            },
        }
    }

    fn set_var(&mut self, value: Vec<stmt::Value>) -> plan::VarId {
        let var = self.var_table.register_var();

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

    fn model(&self, id: impl Into<ModelId>) -> &'a Model {
        self.schema.model(id)
    }
}
