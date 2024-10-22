use stmt::ValueStream;
use toasty_core::*;

use super::*;
use crate::driver::operation;

impl<'stmt> Exec<'stmt> {
    pub(super) async fn exec_insert(&mut self, action: &plan::Insert<'stmt>) -> Result<()> {
        assert!(action.input.is_empty(), "todo");

        let mut stmt = action.stmt.clone();
        let ty = action.output.as_ref().map(|output| output.ty.clone());

        // TODO: bit of a hack, this should be fixed before this point
        if ty.is_none() {
            stmt.returning = None;
        }

        let mut res = self
            .db
            .driver
            .exec(
                &self.db.schema,
                operation::QuerySql {
                    stmt: stmt.into(),
                    ty,
                }
                .into(),
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
                let stmt::Value::Record(record) = value else { todo!() };
                let record = project.eval(&*record)?;
                yield record.into();
            }
        });

        self.vars.store(output.var, res);

        Ok(())
    }
}
