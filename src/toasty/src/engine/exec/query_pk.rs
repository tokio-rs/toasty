use super::*;

impl<'stmt> Exec<'_, 'stmt> {
    pub(super) async fn exec_query_pk(&mut self, action: &plan::QueryPk<'stmt>) -> Result<()> {
        let op = action.apply()?;
        let res = self.db.driver.exec(&self.db.schema, op.into()).await?;

        // TODO: don't clone
        let project = action.project.clone();
        let post_filter = action.post_filter.clone();

        /*
        let res = ValueStream::from_stream(async_stream::try_stream! {
            for await value in res {
                let value = value?;
                let record = project.eval(&value)?;

                if let Some(post_filter) = &post_filter {
                    // TODO: not quite right...
                    let r = record.expect_record();
                    if post_filter.eval_bool(r)? {
                        yield record.into();
                    }
                } else {
                    yield record.into();
                }
            }
        });
        */
        todo!();

        self.vars.store(action.output, res);

        Ok(())
    }
}
