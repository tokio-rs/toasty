use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_query_sql(&mut self, action: &plan::Statement) -> Result<()> {
        let mut sql = action.stmt.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input(input).await?;
            sql.substitute(&[input]);
        }

        let expect_rows = match &sql {
            stmt::Statement::Delete(stmt) => stmt.returning.is_some(),
            stmt::Statement::Insert(stmt) => stmt.returning.is_some(),
            stmt::Statement::Query(_) => true,
            stmt::Statement::Update(stmt) => stmt.returning.is_some(),
        };

        println!("expect_rows={expect_rows:#?}");
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
            Rows::Count(count) => {
                assert!(!expect_rows, "action={action:?}");
                ValueStream::from_stream(async_stream::try_stream! {
                    for _ in 0..count {
                        let row = project.eval_const();
                        yield row;
                    }
                })
            }
            Rows::Values(rows) => {
                assert!(expect_rows, "action={action:?}");
                ValueStream::from_stream(async_stream::try_stream! {
                    for await value in rows {
                        let value = value?;
                        yield project.eval(&[value])?;
                    }
                })
            }
        };

        self.vars.store(out.var, res);

        Ok(())
    }
}
