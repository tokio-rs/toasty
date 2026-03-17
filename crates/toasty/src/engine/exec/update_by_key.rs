use crate::{
    engine::exec::{Action, Exec, Output, VarId},
    Result,
};
use toasty_core::{
    driver::{operation, Rows},
    schema::db::TableId,
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

    /// When `true` return the record being updated *after* the update. When
    /// `false`, just return the count of updated rows.
    pub returning: bool,

    /// Optional guard input. If set and the guard's result is empty, this
    /// operation is skipped (produces empty results).
    pub guard: Option<VarId>,
}

impl Exec<'_> {
    pub(super) async fn action_update_by_key(&mut self, action: &UpdateByKey) -> Result<()> {
        // If guarded and the guard returned empty results, skip execution
        if let Some(guard_var) = action.guard {
            let guard_value = self.vars.load(guard_var).await?.collect_as_value().await?;
            if matches!(&guard_value, stmt::Value::List(items) if items.is_empty()) {
                let res = if action.returning {
                    Rows::value_stream(ValueStream::default())
                } else {
                    Rows::Count(0)
                };
                self.vars
                    .store(action.output.var, action.output.num_uses, res);
                return Ok(());
            }
        }

        let keys = self
            .vars
            .load(action.input)
            .await?
            .collect_as_value()
            .await?
            .unwrap_list();

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

            let res = self.connection.exec(&self.engine.schema, op.into()).await?;

            debug_assert_eq!(!res.rows.is_count(), action.returning);

            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Self {
        Self::UpdateByKey(src)
    }
}
