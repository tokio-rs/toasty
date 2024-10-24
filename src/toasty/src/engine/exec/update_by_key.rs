use super::*;

impl<'stmt> Exec<'_, 'stmt> {
    pub(super) async fn exec_update_by_key(
        &mut self,
        action: &plan::UpdateByKey<'stmt>,
    ) -> Result<()> {
        let op = if let Some(input) = action.input {
            let mut input = self.vars.load(input);

            // Empty input, skip the update
            if input.peek().await.is_none() {
                if let Some(output) = action.output {
                    self.vars.store(output, ValueStream::new());
                }

                return Ok(());
            }

            action.apply_with_input(input).await?
        } else {
            action.apply()?
        };

        let records = self.db.driver.exec(&self.db.schema, op.into()).await?;

        if let Some(output) = action.output {
            self.vars.store(output, records);
        } else {
            let _ = records.collect().await?;
        }

        Ok(())
    }
}
