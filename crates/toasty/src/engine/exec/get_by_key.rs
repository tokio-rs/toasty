use super::{operation, plan, Exec, Result};
use crate::driver::Rows;
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) async fn action_get_by_key(&mut self, action: &plan::GetByKey) -> Result<()> {
        let keys = self
            .eval_keys_maybe_using_input(&action.keys, &action.input)
            .await?;

        let res = if keys.is_empty() {
            ValueStream::default()
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
            let rows = match res.rows {
                Rows::Values(rows) => rows,
                _ => todo!("res={res:#?}"),
            };

            self.project_and_filter_output(
                rows,
                &action.output.project,
                action.post_filter.as_ref(),
            )
        };

        self.vars.store(action.output.var, res);
        Ok(())
    }
}
