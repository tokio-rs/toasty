use crate::{
    Result,
    engine::exec::{Action, Exec, Output, VarId},
};
use toasty_core::{
    driver::operation::QueryPkLimit,
    driver::{ExecResponse, Rows, operation},
    schema::db::{ColumnId, IndexId, TableId},
    stmt,
};

#[derive(Debug)]
pub(crate) struct QueryPk {
    /// Where to get the input
    pub input: Option<VarId>,

    /// Where to store the result
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Optional index to query. None = primary key, Some(id) = secondary index
    pub index: Option<IndexId>,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr,

    /// Filter to pass to the database
    pub row_filter: Option<stmt::Expr>,

    /// Limit and pagination bounds for this query. `None` means unbounded.
    pub limit: Option<QueryPkLimit>,

    /// Sort key ordering direction.
    pub order: Option<stmt::Direction>,
}

impl Exec<'_> {
    pub(super) async fn action_query_pk(&mut self, action: &QueryPk) -> Result<()> {
        let mut pk_filter = action.pk_filter.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input(&[*input]).await?;
            pk_filter.substitute(&input);
        }

        let filters = self.split_filter(pk_filter, action.table);
        let mut all_rows = Vec::new();
        let mut response_cursor = None;

        // Pagination with multiple filters is not supported — a cursor is only
        // meaningful for a single partition key query.
        let has_cursor = matches!(
            &action.limit,
            Some(QueryPkLimit::Cursor { after: Some(_), .. })
        );
        assert!(
            !has_cursor || filters.len() <= 1,
            "cursor-based pagination with multiple partition filters is not supported"
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
                        select: action.columns.clone(),
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

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse {
                values: Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
                next_cursor: response_cursor,
                prev_cursor: None,
            },
        );

        Ok(())
    }
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Action::QueryPk(value)
    }
}
