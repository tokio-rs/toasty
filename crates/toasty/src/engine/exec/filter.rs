use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt,
};

impl Exec<'_> {
    pub(super) async fn exec_filter(&mut self, action: &mir::Filter) -> Result<ExecResponse> {
        // Attached args are loaded whether or not any row is read, keeping
        // use counts exact.
        let args = self.collect_input(action.args.iter().copied()).await?;

        // Load the input variable with metadata
        let input_response = self.vars.load(action.input).await?;
        let mut input_stream = input_response.values.into_value_stream();

        // Predicate input: `arg(0)` = current row, `arg(1 + i)` = `args[i]`.
        let mut eval_input = Vec::with_capacity(1 + args.len());
        eval_input.push(stmt::Value::Null);
        eval_input.extend(args);

        let mut filtered_rows = vec![];

        // Iterate through the input stream and apply the filter
        while let Some(res) = input_stream.next().await {
            eval_input[0] = res?;

            if action
                .predicate
                .eval_bool(&self.engine.schema, &eval_input)?
            {
                filtered_rows.push(std::mem::replace(&mut eval_input[0], stmt::Value::Null));
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
