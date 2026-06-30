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

        // Shred a multi-key update into one single-key op per key so each key's
        // filter is adjudicated independently — matching SQL's per-row
        // semantics, and mirroring how delete fans out. These updates are not
        // atomic.
        let mut total_count = 0u64;
        let mut rows = vec![];

        for key in keys {
            match self.exec_update_one(action, key).await? {
                Rows::Count(n) => total_count += n,
                other => rows.extend(other.into_value_stream().collect().await?),
            }
        }

        // The output shape is a property of the action, not the results: with
        // zero keys there is nothing to match on, yet a `returning` update must
        // still yield an (empty) stream rather than a count.
        let res = if action.returning.is_some() {
            Rows::value_stream(ValueStream::from_vec(rows))
        } else {
            Rows::Count(total_count)
        };

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse::from_rows(res),
        );

        Ok(())
    }

    /// Execute a single-key `UpdateByKey` op for one resolved key.
    async fn exec_update_one(&mut self, action: &UpdateByKey, key: stmt::Value) -> Result<Rows> {
        let op = operation::UpdateByKey {
            table: action.table,
            keys: vec![key],
            assignments: action.assignments.clone(),
            filter: action.filter.clone(),
            condition: action.condition.clone(),
            returning: action.returning.clone(),
        };

        let res = self.connection.exec(&self.engine.schema, op.into()).await?;

        debug_assert_eq!(!res.values.is_count(), action.returning.is_some());

        Ok(res.values)
    }
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Self {
        Self::UpdateByKey(src)
    }
}
