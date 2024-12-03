use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_query_sql(&mut self, action: &plan::QuerySql) -> Result<()> {
        let mut sql = action.stmt.clone();

        if !action.input.is_empty() {
            assert_eq!(action.input.len(), 1);

            let input = self.collect_input(&action.input[0]).await?;
            sql.substitute(&[stmt::Value::List(input)]);
        }

        let res = self
            .db
            .driver
            .exec(&self.db.schema, operation::QuerySql { stmt: sql }.into())
            .await?;

        let Some(out) = &action.output else {
            assert!(res.rows.is_count());
            return Ok(());
        };

        // TODO: don't clone
        let project = out.project.clone();

        let res = match res.rows {
            Rows::Count(count) => ValueStream::from_stream(async_stream::try_stream! {
                for _ in 0..count {
                    let row = project.eval_const();
                    yield row;
                }
            }),
            Rows::Values(rows) => ValueStream::from_stream(async_stream::try_stream! {
                for await value in rows {
                    let value = value?;
                    yield project.eval(&[value])?;
                }
            }),
        };

        self.vars.store(out.var, res);

        Ok(())
    }
}
