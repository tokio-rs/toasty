use super::{plan, Exec, Result};
use crate::driver::Rows;
use toasty_core::{driver::operation, stmt::ValueStream};

impl Exec<'_> {
    pub(super) async fn action_get_by_key2(&mut self, action: &plan::GetByKey2) -> Result<()> {
        let keys = self
            .vars
            .load(action.input)
            .await?
            .into_values()
            .collect()
            .await?;

        let res = if keys.is_empty() {
            Rows::value_stream(ValueStream::default())
        } else {
            let op = operation::GetByKey {
                table: action.table,
                select: action.columns.clone(),
                keys,
            };

            let res = self
                .engine
                .driver
                .exec(&self.engine.schema.db, op.into())
                .await?;
            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);
        Ok(())
    }
}
