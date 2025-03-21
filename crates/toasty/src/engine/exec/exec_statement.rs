use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn action_exec_statement(
        &mut self,
        action: &plan::ExecStatement,
    ) -> Result<()> {
        self.exec_statement(
            action.stmt.clone(),
            action.input.as_ref(),
            action.output.as_ref(),
        )
        .await
    }

    pub(super) async fn exec_statement(
        &mut self,
        stmt: stmt::Statement,
        input: Option<&plan::Input>,
        output: Option<&plan::Output>,
    ) -> Result<()> {
        let mut stmt = stmt.clone();

        if let Some(input) = input {
            let input = self.collect_input(input).await?;
            stmt.substitute(&[input]);
        }

        let expect_rows = match &stmt {
            stmt::Statement::Delete(stmt) => stmt.returning.is_some(),
            stmt::Statement::Insert(stmt) => stmt.returning.is_some(),
            stmt::Statement::Query(_) => true,
            stmt::Statement::Update(stmt) => stmt.returning.is_some(),
        };

        let res = self
            .db
            .driver
            .exec(&self.db.schema.db, operation::QuerySql { stmt }.into())
            .await?;

        let Some(out) = output else {
            assert!(res.rows.is_count());
            return Ok(());
        };

        // TODO: don't clone
        let project = out.project.clone();

        let res = match res.rows {
            Rows::Count(count) => {
                assert!(!expect_rows);
                ValueStream::from_stream(async_stream::try_stream! {
                    for _ in 0..count {
                        let row = project.eval_const();
                        yield row;
                    }
                })
            }
            Rows::Values(rows) => {
                assert!(expect_rows);
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
