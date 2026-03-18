use crate::{
    engine::exec::{Action, Exec, Output, VarId},
    Result,
};
use toasty_core::{
    driver::{operation, Rows},
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

    /// Maximum number of items to return.
    pub limit: Option<i64>,

    /// Sort key ordering direction.
    pub order: Option<stmt::Direction>,

    /// Cursor for resuming a paginated query.
    pub cursor: Option<stmt::Value>,
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
        let mut cursor = None;

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
                        limit: action.limit,
                        order: action.order,
                        cursor: action.cursor.clone(),
                    }
                    .into(),
                )
                .await?;

            let mut stream = res.rows.into_value_stream();
            cursor = stream.take_cursor();
            all_rows.extend(stream.collect().await?);
        }
        eprintln!("cursor: {:?}", cursor);
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            Rows::Stream(stmt::ValueStream::from_vec(all_rows).with_cursor(cursor)),
        );

        Ok(())
    }
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Action::QueryPk(value)
    }
}
