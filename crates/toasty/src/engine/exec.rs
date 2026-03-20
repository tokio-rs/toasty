mod action;
pub(crate) use action::Action;

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod eval;
pub(crate) use eval::Eval;

mod exec_statement;
pub(crate) use exec_statement::{ExecStatement, ExecStatementOutput, PaginationConfig};

mod filter;
pub(crate) use filter::Filter;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

mod kv;

mod nested_merge;
pub(crate) use nested_merge::{
    MergeIndex, MergeQualification, NestedChild, NestedLevel, NestedMerge,
};

mod output;
pub(crate) use output::Output;

mod plan;
pub(crate) use plan::ExecPlan;

mod project;
pub(crate) use project::Project;

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod rmw;
pub(crate) use rmw::ReadModifyWrite;

mod set_var;
pub(crate) use set_var::SetVar;

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

mod var;
pub(crate) use var::{VarDecls, VarId, VarStore};

use crate::{engine::Engine, Result};
use toasty_core::{
    driver::{operation::Transaction, Rows},
    stmt::{self, ValueStream},
    Connection,
};

/// Response from executing an action in the engine pipeline.
/// Carries both the result values and optional pagination metadata.
#[derive(Debug)]
pub(crate) struct ExecResponse {
    /// The result values (rows, count, or stream)
    pub values: Rows,
    /// Cursor to the next page (if paginated and more data exists)
    pub next_cursor: Option<stmt::Value>,
    /// Cursor to the previous page (if backward pagination is supported)
    pub prev_cursor: Option<stmt::Value>,
}

impl ExecResponse {
    /// Create a response from rows with no pagination cursors
    pub fn from_rows(rows: Rows) -> Self {
        Self {
            values: rows,
            next_cursor: None,
            prev_cursor: None,
        }
    }

    /// Convert back to just the rows (discarding cursor metadata)
    pub fn into_rows(self) -> Rows {
        self.values
    }
}

struct Exec<'a> {
    engine: &'a Engine,
    connection: &'a mut dyn Connection,
    vars: VarStore,
    /// True when an outer transaction is active on this connection. Used by
    /// ReadModifyWrite to decide between savepoints (nested) and its own
    /// BEGIN/COMMIT (standalone).
    in_transaction: bool,
}

impl Engine {
    pub(crate) async fn exec_plan(
        &self,
        connection: &mut dyn Connection,
        plan: ExecPlan,
        in_transaction: bool,
    ) -> Result<ExecResponse> {
        let mut exec = Exec {
            engine: self,
            connection,
            vars: plan.vars,
            in_transaction,
        };

        // When nested inside an outer transaction use savepoints so the outer
        // transaction can still commit or roll back as a whole. When standalone,
        // start our own transaction (MySQL requires an active BEGIN before
        // SAVEPOINT can be used, so we can't use savepoints here).
        let (begin, commit, rollback) = if exec.in_transaction {
            let name = "statement";
            (
                Transaction::Savepoint(name.to_owned()),
                Transaction::ReleaseSavepoint(name.to_owned()),
                Transaction::RollbackToSavepoint(name.to_owned()),
            )
        } else {
            (
                Transaction::start(),
                Transaction::Commit,
                Transaction::Rollback,
            )
        };

        if plan.needs_transaction {
            exec.connection.exec(&self.schema, begin.into()).await?;
            exec.in_transaction = true;
        }

        for (i, step) in plan.actions.iter().enumerate() {
            tracing::debug!("Executing action {}: {:?}", i, action_name(step));
            if let Err(e) = exec.exec_step(step).await {
                if plan.needs_transaction {
                    // Best effort: ignore rollback errors so the original error is returned
                    let _ = exec.connection.exec(&self.schema, rollback.into()).await;
                }
                return Err(e);
            }
        }

        if plan.needs_transaction {
            exec.connection.exec(&self.schema, commit.into()).await?;
        }

        let result = if let Some(returning) = plan.returning {
            let response = exec.vars.load(returning).await?;
            tracing::debug!("Final result from var {:?}:\n{:#?}", returning, response);

            let value_stream = match response.values {
                Rows::Count(_) => ValueStream::default(),
                Rows::Value(stmt::Value::List(items)) => ValueStream::from_vec(items),
                // TODO have the public API be able to handle single rows
                Rows::Value(value) => ValueStream::from_vec(vec![value]),
                Rows::Stream(value_stream) => value_stream,
            };

            ExecResponse {
                values: Rows::Stream(value_stream),
                next_cursor: response.next_cursor,
                prev_cursor: response.prev_cursor,
            }
        } else {
            ExecResponse::from_rows(Rows::Stream(ValueStream::default()))
        };

        Ok(result)
    }
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::DeleteByKey(_) => "DeleteByKey",
        Action::Eval(_) => "Eval",
        Action::ExecStatement(_) => "ExecStatement",
        Action::Filter(_) => "Filter",
        Action::FindPkByIndex(_) => "FindPkByIndex",
        Action::GetByKey(_) => "GetByKey",
        Action::NestedMerge(_) => "NestedMerge",
        Action::QueryPk(_) => "QueryPk",
        Action::ReadModifyWrite(_) => "ReadModifyWrite",
        Action::Project(_) => "Project",
        Action::SetVar(_) => "SetVar",
        Action::UpdateByKey(_) => "UpdateByKey",
    }
}

impl Exec<'_> {
    async fn exec_step(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::DeleteByKey(action) => self.action_delete_by_key(action).await,
            Action::Eval(action) => self.action_eval(action).await,
            Action::ExecStatement(action) => self.action_exec_statement(action).await,
            Action::Filter(action) => self.action_filter(action).await,
            Action::FindPkByIndex(action) => self.action_find_pk_by_index(action).await,
            Action::GetByKey(action) => self.action_get_by_key(action).await,
            Action::NestedMerge(action) => self.action_nested_merge(action).await,
            Action::QueryPk(action) => self.action_query_pk(action).await,
            Action::ReadModifyWrite(action) => self.action_read_modify_write(action).await,
            Action::Project(action) => self.action_project(action).await,
            Action::SetVar(action) => self.action_set_var(action),
            Action::UpdateByKey(action) => self.action_update_by_key(action).await,
        }
    }

    async fn collect_input(&mut self, input: &[VarId]) -> Result<Vec<stmt::Value>> {
        let mut ret = Vec::new();

        for var_id in input {
            let response = self.vars.load(*var_id).await?;
            let value = response.values.collect_as_value().await?;
            ret.push(value);
        }

        Ok(ret)
    }
}
