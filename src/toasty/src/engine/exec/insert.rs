use stmt::ValueStream;
use toasty_core::*;

use super::*;
use crate::driver::operation;

impl<'stmt> Exec<'_, 'stmt> {
    pub(super) async fn exec_insert(&mut self, action: &plan::Insert<'stmt>) -> Result<()> {
        assert!(action.input.is_empty(), "todo");

        let mut stmt = action.stmt.clone();

        let mut res = self
            .db
            .driver
            .exec(
                &self.db.schema,
                operation::QuerySql { stmt: stmt.into() }.into(),
            )
            .await?;

        let output = match &action.output {
            Some(output) => output,
            None => {
                // TODO: process in the background
                while let Some(res) = res.next().await {
                    res?;
                }

                return Ok(());
            }
        };

        // TODO: don't clone
        let project = output.project.clone();

        let res = ValueStream::from_stream(async_stream::try_stream! {
            for await value in res {
                let value = value?;
                let record = project.eval(eval::args(&[value][..]))?;
                yield record.into();
            }
        });

        self.vars.store(output.var, res);

        Ok(())
    }
}
