use crate::{
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::driver::Rows;

impl Exec<'_> {
    pub(super) async fn action_filter(&mut self, action: &plan::Filter) -> Result<()> {
        // Load the input variable
        let mut input_stream = self.vars.load(action.input).await?.into_values();

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
