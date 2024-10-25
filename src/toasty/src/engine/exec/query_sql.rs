use super::*;

impl<'stmt> Exec<'_, 'stmt> {
    pub(super) async fn exec_query_sql(&mut self, action: &plan::QuerySql<'stmt>) -> Result<()> {
        let mut sql = action.stmt.clone();

        if !action.input.is_empty() {
            assert_eq!(action.input.len(), 1);

            let input = self.collect_input(&action.input[0]).await?;

            todo!("input = {input:#?}");
        }

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema,
                operation::QuerySql {
                    stmt: sql,
                    ty: action.output.as_ref().map(|o| o.ty.clone()),
                }
                .into(),
            )
            .await?;

        let Some(out) = &action.output else {
            // Should be no output
            let _ = res.collect().await;
            return Ok(());
        };

        // TODO: don't clone
        let project = out.project.clone();

        let res = ValueStream::from_stream(async_stream::try_stream! {
            for await value in res {
                let value = value?;
                yield project.eval(&value)?;
            }
        });

        self.vars.store(out.var, res);

        Ok(())
    }
}
