use crate::{
    driver::Rows,
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::{driver::operation, stmt::ValueStream};

impl Exec<'_> {
    pub(super) async fn action_update_by_key(&mut self, action: &plan::UpdateByKey) -> Result<()> {
        let keys = self
            .vars
            .load(action.input)
            .await?
            .into_values()
            .collect()
            .await?;

        let res = if keys.is_empty() {
            if action.returning {
                Rows::value_stream(ValueStream::default())
            } else {
                Rows::Count(0)
            }
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

            debug_assert_eq!(res.rows.is_values(), action.returning);

            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}
