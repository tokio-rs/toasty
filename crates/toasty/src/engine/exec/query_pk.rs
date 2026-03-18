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

    /// When true, return only the count of matching rows.
    pub count_only: bool,
}

impl Exec<'_> {
    pub(super) async fn action_query_pk(&mut self, action: &QueryPk) -> Result<()> {
        let mut pk_filter = action.pk_filter.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input(&[*input]).await?;
            pk_filter.substitute(&input);
        }

        let filters = self.split_filter(pk_filter, action.table);

        if action.count_only {
            let mut total: i64 = 0;

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
                            count_only: true,
                        }
                        .into(),
                    )
                    .await?;

                total += res.rows.into_count() as i64;
            }

            let record =
                stmt::Value::Record(stmt::ValueRecord::from_vec(vec![stmt::Value::I64(total)]));
            self.vars.store(
                action.output.var,
                action.output.num_uses,
                Rows::Stream(stmt::ValueStream::from_vec(vec![record])),
            );
        } else {
            let mut all_rows = Vec::new();

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
                            count_only: false,
                        }
                        .into(),
                    )
                    .await?;

                all_rows.extend(res.rows.into_value_stream().collect().await?);
            }

            self.vars.store(
                action.output.var,
                action.output.num_uses,
                Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
            );
        }

        Ok(())
    }
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Action::QueryPk(value)
    }
}
