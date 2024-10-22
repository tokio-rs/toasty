use super::*;

impl<'a> Exec<'a> {
    pub(super) async fn exec_delete_by_key(
        &mut self,
        action: &plan::DeleteByKey<'a>,
    ) -> Result<()> {
        let keys = self
            .collect_keys_from_input(&action.keys, &action.input)
            .await?;

        if keys.is_empty() {
            return Ok(());
        } else {
            let op = operation::DeleteByKey {
                table: action.table,
                // TODO: don't eval unecessarily
                keys,
                filter: action.filter.clone(),
            };

            // TODO: do something with the result
            let _ = self
                .db
                .driver
                .exec(&self.db.schema, op.into())
                .await?
                .collect()
                .await?;
        }

        Ok(())
    }
}
