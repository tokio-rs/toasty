use crate::{
    Result,
    engine::{
        exec::Exec,
        mir::{self, LogicalPlan},
    },
};
use toasty_core::driver::{ExecResponse, Rows};

impl Exec<'_> {
    pub(super) async fn exec_eval(
        &mut self,
        logical_plan: &LogicalPlan,
        action: &mir::Eval,
    ) -> Result<ExecResponse> {
        // With a base, the per-row body is erased into a single `map`
        // function over inputs `[base, attached...]`, with pagination
        // metadata forwarding from `base` at input 0.
        let map_func;
        let (inputs, func, metadata) = match action.base {
            Some(base) => {
                let mut inputs = vec![base];
                inputs.extend(action.attached.iter().copied());
                map_func = action.map_func(logical_plan);
                (inputs, &map_func, Some(0))
            }
            None => {
                let inputs: Vec<_> = action.attached.iter().copied().collect();
                (inputs, &action.body, None)
            }
        };

        // Load all input data upfront, preserving pagination metadata
        let mut input = Vec::with_capacity(inputs.len());
        let mut next_cursor = None;
        let mut prev_cursor = None;

        for (i, node_id) in inputs.iter().enumerate() {
            let response = self.vars.load(*node_id).await?;
            let data = response.values.collect_as_value().await?;
            input.push(data);

            if Some(i) == metadata {
                next_cursor = response.next_cursor;
                prev_cursor = response.prev_cursor;
            } else {
                debug_assert!(response.next_cursor.is_none() && response.prev_cursor.is_none());
            }
        }

        // Evaluate the function with the collected inputs
        let result = func.eval(&self.engine.schema, &input)?;

        Ok(ExecResponse {
            values: Rows::Value(result),
            next_cursor,
            prev_cursor,
        })
    }
}
