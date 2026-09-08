use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt::Value,
};

impl Exec<'_> {
    pub(super) async fn exec_eval(&mut self, action: &mir::Eval) -> Result<ExecResponse> {
        match action.row_input {
            Some(row_input) => self.exec_eval_map_over(action, row_input).await,
            None => self.exec_eval_compute(action).await,
        }
    }

    async fn exec_eval_compute(&mut self, action: &mir::Eval) -> Result<ExecResponse> {
        // This form evaluates the body once with each complete input.
        // For example, two input nodes produce this call:
        //
        //     body.eval([input_0, input_1])
        let mut input = Vec::with_capacity(action.inputs.len());

        for &node_id in &action.inputs {
            let response = self.vars.load(node_id).await?;
            // Only a row input can pass page cursors to an `Eval` result.
            debug_assert!(response.is_unpaginated());
            input.push(response.values.collect_as_value().await?);
        }

        let result = action.body.eval(&self.engine.schema, &input)?;

        Ok(ExecResponse::from_rows(Rows::Value(result)))
    }

    async fn exec_eval_map_over(
        &mut self,
        action: &mir::Eval,
        row_input: mir::NodeId,
    ) -> Result<ExecResponse> {
        // This form maps the body over the rows from `row_input`.
        // For example, two rows produce this result:
        //
        //     [row_body.eval(row_input[0], inputs...),
        //      row_body.eval(row_input[1], inputs...)]
        //
        // Load `row_input` first because it decides whether the other inputs
        // are needed.
        let ExecResponse {
            values,
            next_cursor,
            prev_cursor,
        } = self.vars.load(row_input).await?;
        let input_rows = values.collect_as_value().await?;

        if input_rows.is_list_empty() {
            return Ok(self.exec_eval_map_over_empty_input(action, next_cursor, prev_cursor));
        }

        let mut input = Vec::with_capacity(1 + action.inputs.len());
        input.push(input_rows);

        for &node_id in &action.inputs {
            let input_response = self.vars.load(node_id).await?;
            // Only a row input can pass page cursors to an `Eval` result.
            debug_assert!(input_response.is_unpaginated());
            input.push(input_response.values.collect_as_value().await?);
        }

        let result = action.body.eval(&self.engine.schema, &input)?;

        // The output has one item for each input row. Its cursors must match
        // the input cursors so the caller can fetch the next or previous page.
        Ok(ExecResponse {
            values: Rows::Value(result),
            next_cursor,
            prev_cursor,
        })
    }

    fn exec_eval_map_over_empty_input(
        &mut self,
        action: &mir::Eval,
        next_cursor: Option<Box<Value>>,
        prev_cursor: Option<Box<Value>>,
    ) -> ExecResponse {
        // There are no rows to evaluate. Do not load the other inputs.
        // Loading a value consumes one recorded use. Since this path skips
        // those loads, release the uses instead.
        for &node_id in &action.inputs {
            self.vars.release(node_id);
        }

        ExecResponse {
            values: Rows::Value(Value::List(vec![])),
            next_cursor,
            prev_cursor,
        }
    }
}
