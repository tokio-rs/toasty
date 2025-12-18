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
pub(crate) use nested_merge::{MergeQualification, NestedChild, NestedLevel, NestedMerge};

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
    driver::Rows,
    stmt::{self, ValueStream},
};

struct Exec<'a> {
    engine: &'a Engine,
    connection: PoolConnection,
    vars: VarStore,
}

impl Engine {
    pub(crate) async fn exec_plan(&self, plan: ExecPlan) -> Result<ValueStream> {
        let mut exec = Exec {
            engine: self,
            connection: self.pool.get().await?,
            vars: plan.vars,
        };

        for step in &plan.actions {
            exec.exec_step(step).await?;
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
            let value = self.vars.load(*var_id).await?.collect_as_value().await?;
            ret.push(value);
        }

        Ok(ret)
    }
}
