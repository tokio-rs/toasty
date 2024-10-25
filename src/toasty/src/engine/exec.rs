mod associate;
mod batch_write;
mod delete_by_key;
mod find_pk_by_index;
mod get_by_key;
mod insert;
mod query_pk;
mod query_sql;
mod update_by_key;

mod var_store;
pub(crate) use var_store::VarStore;

use crate::{driver::operation, engine::*, Result};

use toasty_core::{sql, stmt};

struct Exec<'a, 'stmt> {
    db: &'a Db,
    vars: VarStore<'stmt>,
}

pub(crate) async fn exec<'stmt>(
    db: &Db,
    pipeline: &plan::Pipeline<'stmt>,
    vars: VarStore<'stmt>,
) -> Result<ValueStream<'stmt>> {
    Exec { db, vars }.exec_pipeline(pipeline).await
}

impl<'a, 'stmt> Exec<'a, 'stmt> {
    async fn exec_pipeline(
        &mut self,
        pipeline: &plan::Pipeline<'stmt>,
    ) -> Result<ValueStream<'stmt>> {
        for step in &pipeline.actions {
            self.exec_step(step).await?;
        }

        Ok(if let Some(returning) = pipeline.returning {
            self.vars.load(returning)
        } else {
            ValueStream::new()
        })
    }

    async fn exec_step(&mut self, action: &Action<'stmt>) -> Result<()> {
        match action {
            Action::Associate(action) => self.exec_associate(action).await,
            Action::BatchWrite(action) => self.exec_batch_write(action).await,
            Action::DeleteByKey(action) => self.exec_delete_by_key(action).await,
            Action::FindPkByIndex(action) => self.exec_find_pk_by_index(action).await,
            Action::GetByKey(action) => self.exec_get_by_key(action).await,
            Action::Insert(action) => self.exec_insert(action).await,
            Action::QueryPk(action) => self.exec_query_pk(action).await,
            Action::QuerySql(action) => self.exec_query_sql(action).await,
            Action::UpdateByKey(action) => self.exec_update_by_key(action).await,
            Action::SetVar(action) => {
                let stream = ValueStream::from_vec(action.value.clone());
                self.vars.store(action.var, stream);
                Ok(())
            }
        }
    }

    async fn collect_input(
        &mut self,
        input: &plan::Input<'stmt>,
    ) -> Result<Vec<stmt::Value<'stmt>>> {
        let mut ret = vec![];

        let mut value_stream = match input.source {
            plan::InputSource::Value(var_id) => self.vars.load(var_id),
            plan::InputSource::Ref(var_id) => self.vars.dup(var_id).await?,
        };

        while let Some(res) = value_stream.next().await {
            let record = res?;

            if let Some(project) = &input.project {
                ret.push(project.eval(&[record][..])?);
            } else {
                ret.push(record);
            }
        }

        Ok(ret)
    }

    async fn collect_keys_from_input(
        &mut self,
        key_expr: &eval::Expr<'stmt>,
        input: &[plan::Input<'stmt>],
    ) -> Result<Vec<stmt::Value<'stmt>>> {
        let mut keys = vec![];

        // For now, there is only one possible input entry
        match &input[..] {
            [input] => {
                for value in self.collect_input(input).await? {
                    let key = key_expr.eval(&[value][..])?;
                    keys.push(key);
                }
            }
            [] => match key_expr {
                eval::Expr::List(expr_list) => {
                    for key_expr in &expr_list.items {
                        keys.push(key_expr.eval_const());
                    }
                }
                key_expr => keys.push(key_expr.eval_const()),
            },
            _ => todo!("key_expr={:#?}; input={:#?}", key_expr, input),
        };

        Ok(keys)
    }
}
