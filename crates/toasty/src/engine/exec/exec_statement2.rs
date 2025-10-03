use super::{operation, plan, Exec, Result};
use crate::driver::Rows;
use toasty_core::stmt;
use toasty_core::stmt::ValueStream;

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
                let values = self.vars.load(*var_id).collect().await?;
                input_values.push(stmt::Value::List(values));
            }
            stmt.substitute(&input_values);
        }

        let op = match &action.output {
            Some(output) => operation::QuerySql {
                stmt,
                ret: Some(output.ty.clone()),
            },
            None => {
                todo!()
            }
        };

        let res = self.db.driver.exec(&self.db.schema.db, op.into()).await?;

        if let Some(output) = &action.output {
            match res.rows {
                Rows::Count(_) => {
                    todo!()
                }
                Rows::Values(rows) => {
                    self.vars.store(output.var, rows);
                }
            }
        } else {
            todo!()
        }

        Ok(())
    }
}
