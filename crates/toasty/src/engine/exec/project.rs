use crate::{
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) async fn action_project(&mut self, action: &plan::Project) -> Result<()> {
        /*
        // Load the input variable
        let mut input_stream = self.vars.dup(action.input).await?;

        // TODO: come up with a more advanced execution task manager to avoid
        // having to eagerly buffer everything.
        let mut projected_rows = vec![];

        for target in &action.output.targets {
            // Stub out a vec for each output target
            projected_rows.push((vec![], target));
        }

        // Iterate through the input stream and project each value
        while let Some(res) = input_stream.next().await {
            let value = res?;

            // Project to each target
            for (projected, target) in &mut projected_rows {
                let row = if target.project.is_identity() {
                    value.clone()
                } else {
                    target.project.eval(&[value.clone()])?
                };
                projected.push(row);
            }
        }

        // Store each projected stream to its target variable
        for (rows, target) in projected_rows {
            self.vars.store(target.var, ValueStream::from_vec(rows));
        }

        Ok(())
        */
        todo!()
    }
}
