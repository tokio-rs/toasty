use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_get_by_key(&mut self, action: &plan::GetByKey) -> Result<()> {
        let args = if let Some(input) = &action.input {
            vec![self.collect_input(input).await?]
        } else {
            vec![]
        };

        println!("eval_keys; fn={:#?}; args={:#?}", action.keys, args);
        let keys = match action.keys.eval(&args[..])? {
            stmt::Value::List(keys) => keys,
            res => todo!("res={res:#?}"),
        };

        let res = if keys.is_empty() {
            ValueStream::new()
        } else {
            let op = operation::GetByKey {
                table: action.table,
                select: action.columns.clone(),
                keys,
            };

            let res = self.db.driver.exec(&self.db.schema, op.into()).await?;
            let rows = match res.rows {
                Rows::Values(rows) => rows,
                _ => todo!("res={res:#?}"),
            };

            // TODO: don't clone
            let output = action.output.clone();
            let post_filter = action.post_filter.clone();

            ValueStream::from_stream(async_stream::try_stream! {
                for await value in rows {
                    let args = [value?];

                    let select = if let Some(filter) = &post_filter {
                        filter.eval_bool(&args)?
                    } else {
                        true
                    };

                    if select {
                        let value = if output.project.is_identity() {
                            let [value] = args else { todo!() };
                            value
                        } else {
                            output.project.eval(&args)?
                        };

                        yield value;
                    }
                }
            })
        };

        self.vars.store(action.output.var, res);
        Ok(())
    }
}
