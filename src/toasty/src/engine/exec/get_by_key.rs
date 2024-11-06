use super::*;

impl<'stmt> Exec<'_, 'stmt> {
    pub(super) async fn exec_get_by_key(&mut self, action: &plan::GetByKey<'stmt>) -> Result<()> {
        // Compute the keys to get
        let keys = self
            .collect_keys_from_input(&action.keys, &action.input)
            .await?;

        let res = if keys.is_empty() {
            ValueStream::new()
        } else {
            let op = operation::GetByKey {
                table: action.table,
                select: action.columns.clone(),
                keys,
                post_filter: action.post_filter.clone(),
            };

            let res = self.db.driver.exec(&self.db.schema, op.into()).await?;

            // TODO: don't clone
            let project = action.project.clone();

            /*
            ValueStream::from_stream(async_stream::try_stream! {
                for await value in res {
                    let value = value?;
                    let value = project.eval(&value)?;
                    yield value.into();
                }
            })
            */
            todo!()
        };

        self.vars.store(action.output, res);
        Ok(())
    }
}
