use crate::{
    Result,
    engine::exec::{Action, Exec, Output, VarId},
};
use toasty_core::{
    driver::operation::Pagination,
    driver::{ExecResponse, Rows, operation},
    schema::db::{ColumnId, TableId},
    stmt,
};

#[derive(Debug)]
pub(crate) struct Scan {
    /// Optional input variable providing runtime args for the filter.
    pub input: Option<VarId>,

    /// Where to store the result.
    pub output: Output,

    /// Table to scan.
    pub table: TableId,

    /// Columns to return.
    pub columns: Vec<ColumnId>,

    /// Filter to apply to each scanned row.
    pub row_filter: Option<stmt::Expr>,

    /// Limit and pagination bounds. `None` means return all rows.
    pub limit: Option<Pagination>,
}

impl Exec<'_> {
    pub(super) async fn action_scan(&mut self, action: &Scan) -> Result<()> {
        let mut row_filter = action.row_filter.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input(&[*input]).await?;
            if let Some(ref mut f) = row_filter {
                f.substitute(&input);
            }
        }

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::Scan {
                    table: action.table,
                    columns: action.columns.iter().map(|col_id| col_id.index).collect(),
                    filter: row_filter,
                    limit: action.limit.clone(),
                }
                .into(),
            )
            .await?;

        let rows: Vec<stmt::Value> = res.values.into_value_stream().collect().await?;

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse {
                values: Rows::Stream(stmt::ValueStream::from_vec(rows)),
                next_cursor: res.next_cursor,
                prev_cursor: None,
            },
        );

        Ok(())
    }
}

impl From<Scan> for Action {
    fn from(value: Scan) -> Self {
        Action::Scan(value)
    }
}
