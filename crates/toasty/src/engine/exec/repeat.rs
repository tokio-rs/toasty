use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt,
};

impl Exec<'_> {
    pub(super) async fn exec_repeat(&mut self, action: &mir::Repeat) -> Result<ExecResponse> {
        let ExecResponse {
            values,
            next_cursor,
            prev_cursor,
        } = self.vars.load(action.input).await?;

        // Only the input's cardinality is observed; a count-only driver
        // response works the same as one that returned rows.
        let count = match values {
            Rows::Count(count) => count as usize,
            Rows::Value(stmt::Value::List(items)) => items.len(),
            Rows::Value(value) => todo!("value={value:#?}"),
            Rows::Stream(mut value_stream) => {
                let mut count = 0;
                while let Some(res) = value_stream.next().await {
                    res?;
                    count += 1;
                }
                count
            }
        };

        let rows = vec![action.value.clone(); count];

        // The output has one item for each input row. Its cursors must match
        // the input cursors so the caller can fetch the next or previous page.
        Ok(ExecResponse {
            values: Rows::value_stream(rows),
            next_cursor,
            prev_cursor,
        })
    }
}
