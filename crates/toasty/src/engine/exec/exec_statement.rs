use super::{operation, plan, Exec, Result};
use crate::driver::Rows;
use toasty_core::stmt;
use toasty_core::stmt::ValueStream;

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
            output.and_then(|out| match out.ty.clone() {
                stmt::Type::Unit => None,
                stmt::Type::Record(fields) => Some(fields),
                _ => todo!(),
            })
        };

        assert_eq!(
            expect_rows,
            ty.is_some(),
            "stmt={stmt:#?}; output={output:#?}"
        );

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema.db,
                operation::QuerySql { stmt, ret: ty }.into(),
            )
            .await?;

        let Some(output) = output else {
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

        // TODO: come up with a more advanced execution task manager to avoid
        // having to eagerly buffer everything.
        let mut projected_rows = vec![];

        for target in &output.targets {
            // Stub out a vec for each output target
            projected_rows.push((vec![], target));
        }

        match res.rows {
            Rows::Count(count) => {
                assert!(!expect_rows);
                for _ in 0..count {
                    for (projected, target) in &mut projected_rows {
                        let row = target.project.eval_const();
                        projected.push(row);
                    }
                }
            }
            Rows::Values(mut rows) => {
                assert!(expect_rows);

                while let Some(res) = rows.next().await {
                    let stmt::Value::Record(record) = res? else {
                        todo!()
                    };
                    for (projected, target) in &mut projected_rows {
                        let row = target.project.eval(&record.fields[..])?;
                        projected.push(row);
                    }
                }
            }
        }

        for (rows, target) in projected_rows {
            self.vars.store(target.var, ValueStream::from_vec(rows));
        }

        Ok(())
    }
}
