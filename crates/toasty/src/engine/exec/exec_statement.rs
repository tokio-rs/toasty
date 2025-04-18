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
            action.conditional_update_with_no_returning,
        )
        .await
    }

    pub(super) async fn exec_statement(
        &mut self,
        mut stmt: stmt::Statement,
        input: Option<&plan::Input>,
        output: Option<&plan::Output>,
        conditional_update_with_no_returning: bool,
    ) -> Result<()> {
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

        let ty = if conditional_update_with_no_returning {
            // Bit of a hack
            Some(vec![stmt::Type::I64, stmt::Type::I64])
        } else {
            output.and_then(|out| match &out.project.args[..] {
                [stmt::Type::Record(fields), ..] => Some(fields.clone()),
                [] => None,
                _ => todo!(),
            })
        };

        assert_eq!(expect_rows, ty.is_some());

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema.db,
                operation::QuerySql { stmt, ret: ty }.into(),
            )
            .await?;

        let Some(out) = output else {
            if conditional_update_with_no_returning {
                let Rows::Values(rows) = res.rows else {
                    return Err(anyhow::anyhow!(
                        "conditional_update_with_no_returning: expected values, got {res:#?}"
                    ));
                };

                let rows = rows.collect().await?;
                assert_eq!(rows.len(), 1);

                let stmt::Value::Record(record) = &rows[0] else {
                    return Err(anyhow::anyhow!(
                        "conditional_update_with_no_returning: expected record, got {rows:#?}"
                    ));
                };
                assert_eq!(record.len(), 2);

                if record[0] == record[1] {
                    return Ok(());
                } else {
                    anyhow::bail!("update condition did not match");
                }
            } else {
                assert!(res.rows.is_count());
                return Ok(());
            }
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
