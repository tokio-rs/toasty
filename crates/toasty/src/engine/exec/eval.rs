use crate::{
    engine::{
        eval,
        exec::{Action, Exec, ExecResponse, Output, VarId},
    },
    Result,
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
        // Load all input data upfront
        let mut input = Vec::with_capacity(action.inputs.len());

        for var_id in &action.inputs {
            let data = self.vars.load(*var_id).await?.collect_as_value().await?;
            input.push(data);
        }

        tracing::debug!(
            "Eval action:\n  Input vars: {:?}\n  Input values:\n{:#?}\n  Function:\n{:#?}",
            action.inputs,
            input,
            action.eval
        );

        // Evaluate the function with the collected inputs
        let result = action.eval.eval(&input)?;

        tracing::debug!("Eval result:\n{:#?}", result);

        // Store the result in the output variable
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse::from_rows(Rows::Value(result)),
        );

        Ok(())
    }
}

impl From<Eval> for Action {
    fn from(value: Eval) -> Self {
        Action::Eval(value)
    }
}
