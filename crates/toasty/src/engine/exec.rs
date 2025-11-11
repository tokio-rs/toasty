mod delete_by_key;
mod exec_statement;
mod filter;
mod find_pk_by_index;
mod get_by_key;
mod nested_merge;
mod project;
mod query_pk;
mod rmw;
mod set_var;
mod update_by_key;

mod var_store;
pub(crate) use var_store::VarStore;

use crate::{
    engine::{
        plan::{self, Action, VarId},
        Engine,
    },
    Result,
};
use toasty_core::stmt::ValueStream;
use toasty_core::{driver::Rows, stmt};

struct Exec<'a> {
    engine: &'a Engine,
    vars: VarStore,
}

impl Engine {
    pub(crate) async fn exec_plan(
        &self,
        pipeline: &plan::Pipeline,
        vars: VarStore,
    ) -> Result<ValueStream> {
        Exec { engine: self, vars }.exec_pipeline(pipeline).await
    }
}

impl Exec<'_> {
    async fn exec_pipeline(&mut self, pipeline: &plan::Pipeline) -> Result<ValueStream> {
        for step in &pipeline.actions {
            self.exec_step(step).await?;
        }

        Ok(if let Some(returning) = pipeline.returning {
            match self.vars.load(returning).await? {
                Rows::Count(_) => ValueStream::default(),
                Rows::Values(value_stream) => value_stream,
            }
        } else {
            ValueStream::default()
        })
    }

    async fn exec_step(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::DeleteByKey(action) => self.action_delete_by_key(action).await,
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

    async fn collect_input2(&mut self, input: &[VarId]) -> Result<Vec<stmt::Value>> {
        let mut ret = Vec::new();

        for var_id in input {
            let values = self
                .vars
                .load(*var_id)
                .await?
                .into_values()
                .collect()
                .await?;
            ret.push(stmt::Value::List(values));
        }

        Ok(ret)
    }
}
