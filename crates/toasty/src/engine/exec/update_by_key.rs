use crate::{
    Result,
    engine::exec::{Action, Exec, Output, VarId},
};
use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    schema::db::{ColumnId, TableId},
    stmt::{self, ValueStream},
};

#[derive(Debug, Clone)]
pub(crate) struct UpdateByKey {
    /// If specified, use the input to generate the list of keys to update
    pub input: VarId,

    /// Where to store the result of the update
    pub output: Output,

    /// Which table to update
    pub table: TableId,

    /// Assignments
    pub assignments: stmt::Assignments,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr>,

    /// Fail the update if the condition is not met
    pub condition: Option<stmt::Expr>,

    /// The columns to return for each updated row *after* the update. When
    /// `None`, just return the count of updated rows.
    pub returning: Option<Vec<ColumnId>>,
}

impl Exec<'_> {
    pub(super) async fn action_update_by_key(&mut self, action: &UpdateByKey) -> Result<()> {
        let keys = self
            .vars
            .load(action.input)
            .await?
            .values
            .collect_as_value()
            .await?
            .into_list_unwrap();

        let res = if keys.is_empty() {
            if action.returning.is_some() {
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
                returning: action.returning.clone(),
            };

            let res = self.connection.exec(&self.engine.schema, op.into()).await?;

            debug_assert_eq!(!res.values.is_count(), action.returning.is_some());

            res.values
        };

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse::from_rows(res),
        );

        Ok(())
    }
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Self {
        Self::UpdateByKey(src)
    }
}
