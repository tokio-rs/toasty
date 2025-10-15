use crate::{
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) async fn action_filter(&mut self, action: &plan::Filter) -> Result<()> {
        // Load the input variable
        let mut input_stream = self.vars.load_count(action.input).await?;

        // TODO: come up with a more advanced execution task manager to avoid
        // having to eagerly buffer everything.
        let mut filtered_rows = vec![];

        // Iterate through the input stream and project each value
        while let Some(res) = input_stream.next().await {
            let value = res?;

            // Apply the filter
            if action.filter.eval_bool(std::slice::from_ref(&value))? {
                filtered_rows.push(value);
            }
        }

        // Store the projected stream to the output variable
        self.vars.store_counted(
            action.output.var,
            action.output.num_uses,
            ValueStream::from_vec(filtered_rows),
        );

        Ok(())
    }
}
