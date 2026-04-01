use crate::{
    Result,
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
};
use toasty_core::driver::{ExecResponse, Rows};

#[derive(Debug)]
pub(crate) struct Eval {
    /// Input sources.
    pub(crate) inputs: Vec<VarId>,

    /// Output variable, where to store the result of the evaluation
    pub(crate) output: Output,

    /// How to evaluate
    pub(crate) eval: eval::Func,

    /// The input from which meta-data should be forwarded. This includes the
    /// pagination cursors. When `None`, do not forward any metadata. Note, all
    /// other inputs must not have any metadata to forward.
    pub(crate) metadata: Option<usize>,
}

impl Exec<'_> {
    pub(super) async fn action_eval(&mut self, action: &Eval) -> Result<()> {
        // Load all input data upfront, preserving pagination metadata
        let mut input = Vec::with_capacity(action.inputs.len());
        let mut next_cursor = None;
        let mut prev_cursor = None;

        for (i, var_id) in action.inputs.iter().enumerate() {
            let response = self.vars.load(*var_id).await?;
            let data = response.values.collect_as_value().await?;
            input.push(data);

            if Some(i) == action.metadata {
                next_cursor = response.next_cursor;
                prev_cursor = response.prev_cursor;
            } else {
                debug_assert!(response.next_cursor.is_none() && response.prev_cursor.is_none());
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
