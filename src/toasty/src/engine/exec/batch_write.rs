use super::*;

impl Exec<'_> {
    pub(super) async fn action_batch_write(&mut self, action: &plan::BatchWrite) -> Result<()> {
        // TODO: actually batch!
        for step in &action.items {
            match step {
                plan::WriteAction::DeleteByKey(action) => self.action_delete_by_key(action).await?,
                plan::WriteAction::Insert(action) => self.action_insert(action).await?,
                plan::WriteAction::UpdateByKey(action) => self.action_update_by_key(action).await?,
            }
        }

        Ok(())
    }
}
