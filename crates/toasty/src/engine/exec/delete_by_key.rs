use toasty_core::driver::{operation, Rows};

use super::{plan, Exec, Result};

impl Exec<'_> {
    pub(super) async fn action_delete_by_key(&mut self, action: &plan::DeleteByKey) -> Result<()> {
        let keys = self
            .vars
            .load(action.input)
            .await?
            .into_values()
            .collect()
            .await?;

        let res = if keys.is_empty() {
            Rows::Count(0)
        } else {
            let op = operation::DeleteByKey {
                table: action.table,
                keys,
                filter: action.filter.clone(),
            };

            let res = self
                .engine
                .driver
                .exec(&self.engine.schema.db, op.into())
                .await?;

            assert!(res.rows.is_count(), "TODO");
            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}
