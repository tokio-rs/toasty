use crate::{
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::driver::Rows;

impl Exec<'_> {
    pub(super) async fn action_project(&mut self, action: &plan::Project) -> Result<()> {
        // TODO: come up with a more advanced execution task manager to avoid
        // having to eagerly buffer everything.
        let mut projected_rows = vec![];

        match self.vars.load_count(action.input).await? {
            Rows::Values(mut value_stream) => {
                while let Some(res) = value_stream.next().await {
                    let value = res?;

                    // Apply the projection
                    let row = action.projection.eval(&[value])?;
                    projected_rows.push(row);
                }
            }
            Rows::Count(count) => {
                for _ in 0..count {
                    let row = action.projection.eval_const();
                    projected_rows.push(row);
                }
            }
        }

        // Store the projected stream to the output variable
        self.vars.store_counted(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(projected_rows),
        );

        Ok(())
    }
}
