use super::{operation, plan, Exec, Result};
use toasty_core::{driver::Rows, stmt};

impl Exec<'_> {
    pub(super) async fn action_exec_statement2(
        &mut self,
        action: &plan::ExecStatement2,
    ) -> Result<()> {
        // TODO: make this parallel

        let mut stmt = action.stmt.clone();

        // Collect input values and substitute into the statement
        if !action.input.is_empty() {
            let mut input_values = Vec::new();
            for var_id in &action.input {
                let values = self
                    .vars
                    .load_count(*var_id)
                    .await?
                    .into_values()
                    .collect()
                    .await?;
                input_values.push(stmt::Value::List(values));
            }
            stmt.substitute(&input_values);
        }

        debug_assert!(
            stmt.returning()
                .and_then(|returning| returning.as_expr())
                .map(|expr| expr.is_record())
                .unwrap_or(true),
            "stmt={stmt:#?}"
        );

        let op = operation::QuerySql {
            stmt,
            ret: if action.conditional_update_with_no_returning {
                Some(vec![stmt::Type::I64, stmt::Type::I64])
            } else {
                action.output.ty.clone()
            },
        };

        let mut res = self
            .engine
            .driver
            .exec(&self.engine.schema.db, op.into())
            .await?;

        if action.conditional_update_with_no_returning {
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

            if record[0] != record[1] {
                anyhow::bail!("update condition did not match");
            }

            res.rows = Rows::Count(record[0].to_u64_unwrap());
        }

        self.vars.store_counted(
            action.output.output.var,
            action.output.output.num_uses,
            res.rows,
        );

        Ok(())
    }
}
