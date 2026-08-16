use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::driver::{ExecResponse, Rows};

impl Exec<'_> {
    pub(super) async fn exec_filter(&mut self, action: &mir::Filter) -> Result<ExecResponse> {
        // Load the input variable with metadata
        let input_response = self.vars.load(action.input).await?;
        let mut input_stream = input_response.values.into_value_stream();

        let mut filtered_rows = vec![];

        // Iterate through the input stream and apply the filter
        while let Some(res) = input_stream.next().await {
            let value = res?;

            if action
                .predicate
                .eval_bool(&self.engine.schema, std::slice::from_ref(&value))?
            {
                filtered_rows.push(value);
            }
        }

        // Return the filtered stream with preserved pagination metadata
        Ok(ExecResponse {
            values: Rows::value_stream(filtered_rows),
            next_cursor: input_response.next_cursor,
            prev_cursor: input_response.prev_cursor,
        })
    }
}
