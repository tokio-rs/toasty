use crate::{
    Result,
    engine::{
        eval,
        exec::{Action, Exec, ExecResponse, Output, VarId},
    },
};
use toasty_core::driver::Rows;

#[derive(Debug)]
pub(crate) struct Eval {
    /// Input sources.
    pub(crate) inputs: Vec<VarId>,

    /// Output variable, where to store the result of the evaluation
    pub(crate) output: Output,

    /// How to evaluate
    pub(crate) eval: eval::Func,
}

impl Exec<'_> {
    pub(super) async fn action_eval(&mut self, action: &Eval) -> Result<()> {
        // Load all input data upfront, preserving pagination metadata
        let mut input = Vec::with_capacity(action.inputs.len());
        let mut next_cursor = None;
        let mut prev_cursor = None;

        for var_id in &action.inputs {
            let response = self.vars.load(*var_id).await?;
            let data = response.values.collect_as_value().await?;
            input.push(data);

            // Preserve pagination cursors from any input that has them
            // (typically only one input will have cursors - the paginated query result)
            if response.next_cursor.is_some() {
                next_cursor = response.next_cursor;
            }
            if response.prev_cursor.is_some() {
                prev_cursor = response.prev_cursor;
            }
        }

        // Evaluate the function with the collected inputs
        let result = action.eval.eval(&input)?;

        // Store the result in the output variable with preserved pagination metadata
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse {
                values: Rows::Value(result),
                next_cursor,
                prev_cursor,
            },
        );

        Ok(())
    }
}

impl From<Eval> for Action {
    fn from(value: Eval) -> Self {
        Action::Eval(value)
    }
}
