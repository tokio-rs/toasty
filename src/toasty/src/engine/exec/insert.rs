use stmt::ValueStream;
use toasty_core::*;

use super::*;
use crate::driver::operation;

impl<'stmt> Exec<'stmt> {
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
                assert!(action.stmt.returning.is_none());
                return Ok(());
            }
        };

        let rows = res.rows.into_values();

        // TODO: don't clone
        let project = output.project.clone();

        let res = ValueStream::from_stream(async_stream::try_stream! {
            for await value in rows {
                let value = value?;
                let record = project.eval(eval::args(&[value][..]))?;
                yield record.into();
            }
        });

        self.vars.store(output.var, res);

        Ok(())
    }
}
