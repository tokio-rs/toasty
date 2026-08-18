use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    schema::db::ColumnId,
    stmt::{self, ValueStream},
};

impl Exec<'_> {
    pub(super) async fn exec_update_by_key(
        &mut self,
        action: &mir::UpdateByKey,
    ) -> Result<ExecResponse> {
        // The columns to return for each updated row *after* the update.
        // `None` means just return the count of updated rows.
        let returning = if action.ty.is_unit() {
            None
        } else {
            Some(mir::column_ids(action.table, &action.columns).collect())
        };

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
            match self.exec_update_one(action, &returning, key).await? {
                Rows::Count(n) => total_count += n,
                other => rows.extend(other.into_value_stream().collect().await?),
            }
        }

        // The output shape is a property of the action, not the results: with
        // zero keys there is nothing to match on, yet a `returning` update must
        // still yield an (empty) stream rather than a count.
        let res = if returning.is_some() {
            Rows::value_stream(ValueStream::from_vec(rows))
        } else {
            Rows::Count(total_count)
        };

        Ok(ExecResponse::from_rows(res))
    }

    /// Execute a single-key `UpdateByKey` op for one resolved key.
    async fn exec_update_one(
        &mut self,
        action: &mir::UpdateByKey,
        returning: &Option<Vec<ColumnId>>,
        key: stmt::Value,
    ) -> Result<Rows> {
        let op = operation::UpdateByKey {
            table: action.table,
            keys: vec![key],
            assignments: action.assignments.clone(),
            filter: action.filter.clone(),
            condition: action.condition.clone(),
            returning: returning.clone(),
        };

        let res = self.connection.exec(&self.engine.schema, op.into()).await?;

        debug_assert_eq!(!res.values.is_count(), returning.is_some());

        Ok(res.values)
    }
}
