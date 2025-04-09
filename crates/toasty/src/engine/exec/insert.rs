use toasty_core::*;

use super::*;

impl Exec<'_> {
    // TODO: unify w/ exec_query_sql
    pub(super) async fn action_insert(&mut self, action: &plan::Insert) -> Result<()> {
        self.exec_statement(
            action.stmt.clone().into(),
            action.input.as_ref(),
            action.output.as_ref(),
            false,
        )
        .await
    }
}
