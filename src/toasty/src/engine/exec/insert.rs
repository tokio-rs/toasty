use stmt::ValueStream;
use toasty_core::*;

use super::*;
use crate::driver::operation;

impl Exec<'_> {
    pub(super) async fn exec_insert(&mut self, action: &plan::Insert) -> Result<()> {
        assert!(action.input.is_none(), "todo");

        let mut stmt = action.stmt.clone();

        let mut res = self
            .db
            .driver
            .exec(
                &self.db.schema,
                operation::QuerySql { stmt: stmt.into() }.into(),
            )
            .await?;

        let Some(output) = &action.output else {
            assert!(action.stmt.returning.is_none());
            return Ok(());
        };

        let res = self.project_and_filter_output(res.rows.into_values(), &output.project, None);
        self.vars.store(output.var, res);

        Ok(())
    }
}
