use crate::{
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
    Result,
};
use toasty_core::driver::Rows;

#[derive(Debug)]
pub(crate) struct Filter {
    /// Source of the input
    pub(crate) input: VarId,

    /// Where to store the output
    pub(crate) output: Output,

    /// How to project it before storing
    pub(crate) filter: eval::Func,
}

impl Exec<'_> {
    pub(super) async fn action_filter(&mut self, action: &Filter) -> Result<()> {
        // Load the input variable
        let mut input_stream = self.vars.load(action.input).await?.into_value_stream();

        let mut filtered_rows = vec![];

        // Iterate through the input stream and apply the filter
        while let Some(res) = input_stream.next().await {
            let value = res?;

            if action.filter.eval_bool(std::slice::from_ref(&value))? {
                filtered_rows.push(value);
            }
        }

        // Store the filtered stream to the output variable
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(filtered_rows),
        );

        Ok(())
    }
}

impl From<Filter> for Action {
    fn from(value: Filter) -> Self {
        Action::Filter(value)
    }
}
