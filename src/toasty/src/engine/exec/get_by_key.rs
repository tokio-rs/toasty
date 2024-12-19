use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_get_by_key(&mut self, action: &plan::GetByKey) -> Result<()> {
        let args = if let Some(input) = &action.input {
            vec![self.collect_input(input).await?]
        } else {
            vec![]
        };

        let keys = match action.keys.eval(&args[..])? {
            stmt::Value::List(keys) => keys,
            res => todo!("res={res:#?}"),
        };

        let res = if keys.is_empty() {
            ValueStream::new()
        } else {
            assert!(action.post_filter.is_none());

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
            assert!(action.post_filter.is_none());

            ValueStream::from_stream(async_stream::try_stream! {
                for await value in rows {
                    let value = value?;
                    let value = output.project.eval(&[value])?;
                    yield value;
                }
            })
        };

        self.vars.store(action.output.var, res);
        Ok(())
    }
}
