use crate::{
    Result,
    engine::{
        eval,
        exec::Exec,
        mir::{self, LogicalPlan},
    },
};
use toasty_core::driver::{ExecResponse, Rows};

impl Exec<'_> {
    pub(super) async fn exec_compute(&mut self, action: &mir::Compute) -> Result<ExecResponse> {
        let inputs: Vec<_> = action.inputs.iter().copied().collect();
        self.eval_func(&inputs, &action.body, None).await
    }

    pub(super) async fn exec_map_over(
        &mut self,
        logical_plan: &LogicalPlan,
        action: &mir::MapOver,
    ) -> Result<ExecResponse> {
        let mut inputs = vec![action.base];
        inputs.extend(action.attached.iter().copied());

        let func = action.eval_func(logical_plan);

        // Metadata forwards from `base`, always input 0.
        self.eval_func(&inputs, &func, Some(0)).await
    }

    /// Evaluates `func` over the collected whole values of `inputs`,
    /// forwarding pagination metadata from the input at position `metadata`
    /// (all other inputs must have none to forward).
    async fn eval_func(
        &mut self,
        inputs: &[mir::NodeId],
        func: &eval::Func,
        metadata: Option<usize>,
    ) -> Result<ExecResponse> {
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
