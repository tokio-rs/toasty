use super::{operation, plan, Exec, Result};
use crate::driver::Rows;
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) async fn action_update_by_key(&mut self, action: &plan::UpdateByKey) -> Result<()> {
        let keys = self
            .vars
            .load_count(action.input)
            .await?
            .into_values()
            .collect()
            .await?;

        let res = if keys.is_empty() {
            Rows::value_stream(ValueStream::default())
        } else {
            let op = operation::UpdateByKey {
                table: action.table,
                keys,
                assignments: action.assignments.clone(),
                filter: action.filter.clone(),
                condition: action.condition.clone(),
                returning: action.returning,
            };

            let res = self
                .engine
                .driver
                .exec(&self.engine.schema.db, op.into())
                .await?;

            res.rows
        };

        self.vars
            .store_counted(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}
