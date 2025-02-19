use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn action_update_by_key(&mut self, action: &plan::UpdateByKey) -> Result<()> {
        let args = if let Some(input) = &action.input {
            vec![self.collect_input(input).await?]
        } else {
            vec![]
        };

        let keys = match action.keys.eval(&args[..])? {
            stmt::Value::List(keys) => keys,
            res => todo!("res={res:#?}"),
        };

        if keys.is_empty() {
            if let Some(output) = &action.output {
                self.vars.store(output.var, ValueStream::default());
            }
        } else {
            let op = operation::UpdateByKey {
                table: action.table,
                keys,
                assignments: action.assignments.clone(),
                filter: action.filter.clone(),
                condition: action.condition.clone(),
                // TODO: not actually correct
                returning: action.output.is_some(),
            };

            let res = self.db.driver.exec(&self.db.schema.db, op.into()).await?;

            match res.rows {
                Rows::Values(rows) => {
                    let Some(output) = &action.output else {
                        todo!("action={action:#?}");
                    };

                    let res = self.project_and_filter_output(rows, &output.project, None);
                    self.vars.store(output.var, res);
                }
                Rows::Count(_) => {
                    debug_assert!(action.output.is_none());
                }
            }
        }

        Ok(())
    }
}
