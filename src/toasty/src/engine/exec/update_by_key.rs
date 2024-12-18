use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_update_by_key(&mut self, action: &plan::UpdateByKey) -> Result<()> {
        let args = if let Some(input) = &action.input {
            vec![self.collect_input(input).await?]
        } else {
            vec![]
        };

        let keys = match action.keys.eval(&args[..])? {
            stmt::Value::List(keys) => keys,
            res => todo!("res={res:#?}"),
        };

        if keys.is_empty() {
            if let Some(output) = &action.output {
                self.vars.store(output.var, ValueStream::new());
            }
        } else {
            let op = operation::UpdateByKey {
                table: action.table,
                keys,
                assignments: action.assignments.clone(),
                filter: action.filter.clone(),
                condition: action.condition.clone(),
                // TODO: not actually correct
                returning: action.output.is_some(),
            };

            let res = self.db.driver.exec(&self.db.schema, op.into()).await?;

            match res.rows {
                Rows::Values(rows) => {
                    let Some(output) = &action.output else {
                        todo!()
                    };

                    let res = if output.project.is_identity() {
                        rows
                    } else {
                        let project = output.project.clone();

                        ValueStream::from_stream(async_stream::try_stream! {
                            for await value in rows {
                                let value = value?;
                                let value = project.eval(&[value])?;
                                yield value;
                            }
                        })
                    };

                    self.vars.store(output.var, res);
                }
                Rows::Count(count) => {
                    debug_assert!(action.output.is_none());
                }
            }
        }

        Ok(())
    }
}
