use crate::{
    Result,
    engine::{
        exec::Exec,
        mir::{self, LogicalPlan},
    },
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt::Value,
};

impl Exec<'_> {
    pub(super) async fn exec_eval(
        &mut self,
        logical_plan: &LogicalPlan,
        action: &mir::Eval,
    ) -> Result<ExecResponse> {
        match action.base {
            Some(base) => self.exec_eval_map_over(logical_plan, action, base).await,
            None => self.exec_eval_compute(action).await,
        }
    }

    async fn exec_eval_compute(&mut self, action: &mir::Eval) -> Result<ExecResponse> {
        // This form evaluates the body once with each complete input.
        // For example, two attached nodes produce this call:
        //
        //     body.eval([attached_0, attached_1])
        let mut input = Vec::with_capacity(action.attached.len());

        for &node_id in &action.attached {
            let response = self.vars.load(node_id).await?;
            // Only a base can pass page cursors to an `Eval` result.
            debug_assert!(response.is_unpaginated());
            input.push(response.values.collect_as_value().await?);
        }

        let result = action.body.eval(&self.engine.schema, &input)?;

        Ok(ExecResponse::from_rows(Rows::Value(result)))
    }

    async fn exec_eval_map_over(
        &mut self,
        logical_plan: &LogicalPlan,
        action: &mir::Eval,
        base: mir::NodeId,
    ) -> Result<ExecResponse> {
        // This form evaluates the body once for each row from `base`.
        // For example, two base rows produce this result:
        //
        //     [body.eval(base[0], attached...),
        //      body.eval(base[1], attached...)]
        //
        // Load `base` first because it decides whether the attached inputs
        // are needed.
        let ExecResponse {
            values,
            next_cursor,
            prev_cursor,
        } = self.vars.load(base).await?;
        let base_rows = values.collect_as_value().await?;

        if base_rows.is_list_empty() {
            return Ok(self.exec_eval_map_over_empty_base(action, next_cursor, prev_cursor));
        }

        let mut input = Vec::with_capacity(1 + action.attached.len());
        input.push(base_rows);

        for &node_id in &action.attached {
            let attached_response = self.vars.load(node_id).await?;
            // Only a base can pass page cursors to an `Eval` result.
            debug_assert!(attached_response.is_unpaginated());
            input.push(attached_response.values.collect_as_value().await?);
        }

        let map_func = action.map_func(logical_plan);
        let result = map_func.eval(&self.engine.schema, &input)?;

        // The output has one item for each base row. Its cursors must match
        // the base cursors so the caller can fetch the next or previous page.
        Ok(ExecResponse {
            values: Rows::Value(result),
            next_cursor,
            prev_cursor,
        })
    }

    fn exec_eval_map_over_empty_base(
        &mut self,
        action: &mir::Eval,
        next_cursor: Option<Box<Value>>,
        prev_cursor: Option<Box<Value>>,
    ) -> ExecResponse {
        // There are no rows to evaluate. Do not load the attached inputs.
        // Loading a value consumes one recorded use. Since this path skips
        // those loads, release the uses instead.
        for &node_id in &action.attached {
            self.vars.release(node_id);
        }

        ExecResponse {
            values: Rows::Value(Value::List(vec![])),
            next_cursor,
            prev_cursor,
        }
    }
}
