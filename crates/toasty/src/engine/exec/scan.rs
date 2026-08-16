use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    stmt,
};

impl Exec<'_> {
    pub(super) async fn exec_scan(&mut self, action: &mir::Scan) -> Result<ExecResponse> {
        let mut row_filter = action.row_filter.clone();

        if let Some(input) = action.input {
            let input = self.collect_input([input]).await?;
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
                    columns: mir::column_ids(action.table, &action.columns)
                        .iter()
                        .map(|col_id| col_id.index)
                        .collect(),
                    filter: row_filter,
                    limit: action.limit.clone(),
                }
                .into(),
            )
            .await?;

        let rows: Vec<stmt::Value> = res.values.into_value_stream().collect().await?;

        Ok(ExecResponse {
            values: Rows::Stream(stmt::ValueStream::from_vec(rows)),
            next_cursor: res.next_cursor,
            prev_cursor: None,
        })
    }
}
