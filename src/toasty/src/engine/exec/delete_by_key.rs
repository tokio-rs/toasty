use super::*;

impl Exec<'_> {
    pub(super) async fn action_delete_by_key(&mut self, action: &plan::DeleteByKey) -> Result<()> {
        let keys = self
            .eval_keys_maybe_using_input(&action.keys, &action.input)
            .await?;

        if keys.is_empty() {
            return Ok(());
        } else {
            let op = operation::DeleteByKey {
                table: action.table,
                keys,
                filter: action.filter.clone(),
            };

            let res = self.db.driver.exec(&self.db.schema.db, op.into()).await?;
            assert!(res.rows.is_count(), "TODO");
        }

        Ok(())
    }
}
