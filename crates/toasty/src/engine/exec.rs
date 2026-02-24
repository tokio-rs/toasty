mod action;
pub(crate) use action::Action;

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod eval;
pub(crate) use eval::Eval;

mod exec_statement;
pub(crate) use exec_statement::{ExecStatement, ExecStatementOutput};

mod filter;
pub(crate) use filter::Filter;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

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

use crate::{db::PoolConnection, engine::Engine, Result};
use toasty_core::{
    driver::{operation::Transaction, Rows},
    stmt::{self, ValueStream},
};

struct Exec<'a> {
    engine: &'a Engine,
    connection: PoolConnection,
    vars: VarStore,
    /// Monotonically increasing counter for generating unique savepoint IDs
    /// within a single plan execution.
    next_savepoint_id: usize,
}

impl Engine {
    pub(crate) async fn exec_plan(&self, plan: ExecPlan) -> Result<ValueStream> {
        let mut exec = Exec {
            engine: self,
            connection: self.pool.get().await?,
            vars: plan.vars,
            next_savepoint_id: 0,
        };

        if plan.needs_transaction {
            exec.connection
                .exec(&self.schema.db, Transaction::Start.into())
                .await?;
        }

        for step in &plan.actions {
            if let Err(e) = exec.exec_step(step).await {
                if plan.needs_transaction {
                    // Best effort: ignore rollback errors so the original error is returned
                    let _ = exec
                        .connection
                        .exec(&self.schema.db, Transaction::Rollback.into())
                        .await;
                }
                return Err(e);
            }
        }

        if plan.needs_transaction {
            exec.connection
                .exec(&self.schema.db, Transaction::Commit.into())
                .await?;
        }

        Ok(if let Some(returning) = plan.returning {
            match exec.vars.load(returning).await? {
                Rows::Count(_) => ValueStream::default(),
                Rows::Value(stmt::Value::List(items)) => ValueStream::from_vec(items),
                // TODO have the public API be able to handle single rows
                Rows::Value(value) => ValueStream::from_vec(vec![value]),
                Rows::Stream(value_stream) => value_stream,
            }
        } else {
            ValueStream::default()
        })
    }
}

/// If `expr` is `ANY(MAP(Value::List([...]), pred))`, returns the list items and predicate
/// template. Returns `None` for any other form, including the batch-load `ANY(MAP(arg[i], pred))`
/// where the base has not yet been substituted.
fn try_extract_any_map_list(expr: &stmt::Expr) -> Option<(&[stmt::Value], &stmt::Expr)> {
    let stmt::Expr::Any(any) = expr else {
        return None;
    };
    let stmt::Expr::Map(map) = &*any.expr else {
        return None;
    };
    let stmt::Expr::Value(stmt::Value::List(items)) = &*map.base else {
        return None;
    };
    Some((items, &map.map))
}

impl Exec<'_> {
    fn generate_savepoint_id(&mut self) -> usize {
        let id = self.next_savepoint_id;
        self.next_savepoint_id += 1;
        id
    }

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
            let value = self.vars.load(*var_id).await?.collect_as_value().await?;
            ret.push(value);
        }

        Ok(ret)
    }
}
