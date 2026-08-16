use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    stmt,
};

impl Exec<'_> {
    pub(super) async fn exec_query_pk(&mut self, action: &mir::QueryPk) -> Result<ExecResponse> {
        let mut pk_filter = action.pk_filter.clone();

        if let Some(input) = action.input {
            let input = self.collect_input([input]).await?;
            pk_filter.substitute(&input);
        }

        let filters = self.split_filter(pk_filter, action.table);
        let mut all_rows = Vec::new();
        let mut response_cursor = None;

        // A limit or pagination clause is only meaningful against a single
        // partition key query. With multiple filters, each partition call
        // would apply the limit/offset independently and produce wrong
        // totals (e.g. `.limit(10)` across 3 partitions could return 30
        // rows, each offset skipping within its own partition).
        assert!(
            action.limit.is_none() || filters.len() <= 1,
            "limit/pagination with multiple partition filters is not supported; filters={}",
            filters.len()
        );

        // When there are multiple filters, discard the response cursor since it
        // would only apply to the last filter's result set.
        let paginated = filters.len() <= 1;

        for f in filters {
            let res = self
                .connection
                .exec(
                    &self.engine.schema,
                    operation::QueryPk {
                        table: action.table,
                        index: action.index,
                        select: mir::column_ids(action.table, &action.columns),
                        pk_filter: f,
                        filter: action.row_filter.clone(),
                        limit: action.limit.clone(),
                        order: action.order,
                    }
                    .into(),
                )
                .await?;

            // Only capture cursor when paginating a single filter
            if paginated && res.next_cursor.is_some() {
                response_cursor = res.next_cursor;
            }

            all_rows.extend(res.values.into_value_stream().collect().await?);
        }

        Ok(ExecResponse {
            values: Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
            next_cursor: response_cursor,
            prev_cursor: None,
        })
    }
}
