mod associate;
mod batch_write;
mod delete_by_key;
mod exec_statement;
mod exec_statement2;
mod filter;
mod find_pk_by_index;
mod get_by_key;
mod insert;
mod kv;
mod nested_merge;
mod project;
mod query_pk;
mod rmw;
mod set_var;
mod update_by_key;

mod var_store;
pub(crate) use var_store::VarStore;

use crate::{
    driver::operation,
    engine::{
        eval,
        plan::{self, Action, VarId},
        Engine,
    },
    Result,
};
use toasty_core::stmt;
use toasty_core::stmt::ValueStream;

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
            self.vars.load(returning)
        } else {
            ValueStream::default()
        })
    }

    async fn exec_step(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::Associate(action) => self.action_associate(action).await,
            Action::BatchWrite(action) => self.action_batch_write(action).await,
            Action::DeleteByKey(action) => self.action_delete_by_key(action).await,
            Action::ExecStatement(action) => self.action_exec_statement(action).await,
            Action::ExecStatement2(action) => self.action_exec_statement2(action).await,
            Action::Filter(action) => self.action_filter(action).await,
            Action::FindPkByIndex(action) => self.action_find_pk_by_index(action).await,
            Action::FindPkByIndex2(action) => self.action_find_pk_by_index2(action).await,
            Action::GetByKey(action) => self.action_get_by_key(action).await,
            Action::GetByKey2(action) => self.action_get_by_key2(action).await,
            Action::Insert(action) => self.action_insert(action).await,
            Action::NestedMerge(action) => self.action_nested_merge(action).await,
            Action::QueryPk(action) => self.action_query_pk(action).await,
            Action::QueryPk2(action) => self.action_query_pk2(action).await,
            Action::ReadModifyWrite(action) => self.action_read_modify_write(action).await,
            Action::ReadModifyWrite2(action) => self.action_read_modify_write2(action).await,
            Action::Project(action) => self.action_project(action).await,
            Action::SetVar(action) => {
                self.vars
                    .store(action.var, ValueStream::from_vec(action.value.clone()));
                Ok(())
            }
            Action::SetVar2(action) => self.action_set_var2(action),
            Action::UpdateByKey(action) => self.action_update_by_key(action).await,
        }
    }

    async fn collect_input(&mut self, input: &plan::Input) -> Result<stmt::Value> {
        let value_stream = match input.source {
            plan::InputSource::Value(var_id) => self.vars.load(var_id),
        };

        let mut values = stmt::Value::List(value_stream.collect().await?);

        if !input.project.is_identity() {
            values = input.project.eval(&[values])?;
        }

        Ok(values)
    }

    async fn collect_input2(&mut self, input: &[VarId]) -> Result<Vec<stmt::Value>> {
        let mut ret = Vec::new();

        for var_id in input {
            let values = self.vars.load_count(*var_id).await?.collect().await?;
            ret.push(stmt::Value::List(values));
        }

        Ok(ret)
    }
}
