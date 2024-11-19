use super::*;

impl<'stmt> Exec<'stmt> {
    pub(super) async fn exec_batch_write(
        &mut self,
        action: &plan::BatchWrite<'stmt>,
    ) -> Result<()> {
        // TODO: actually batch!
        for step in &action.items {
            match step {
                plan::WriteAction::DeleteByKey(action) => self.exec_delete_by_key(action).await?,
                plan::WriteAction::Insert(action) => self.exec_insert(action).await?,
                plan::WriteAction::UpdateByKey(action) => self.exec_update_by_key(action).await?,
            }
        }

        Ok(())
    }
}
