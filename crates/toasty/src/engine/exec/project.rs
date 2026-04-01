use crate::{
    Result,
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt,
};

#[derive(Debug)]
pub(crate) struct Project {
    /// Source of the input
    pub(crate) input: VarId,

    /// Where to store the output
    pub(crate) output: Output,

    /// How to project it before storing
    pub(crate) projection: eval::Func,
}

impl Exec<'_> {
    pub(super) async fn action_project(&mut self, action: &Project) -> Result<()> {
        // TODO: come up with a more advanced execution task manager to avoid
        // having to eagerly buffer everything.
        let mut projected_rows = vec![];

        // Load input with full metadata to preserve pagination cursors
        let input_response = self.vars.load(action.input).await?;

        match input_response.values {
            Rows::Value(value) => {
                match value {
                    stmt::Value::List(items) => {
                        for value in items {
                            // Apply the projection
                            let row = action.projection.eval(&[value])?;
                            projected_rows.push(row);
                        }
                    }
                    _ => todo!("value={value:#?}"),
                }
            }
            Rows::Stream(mut value_stream) => {
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

        // Store the projected stream with preserved pagination metadata
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse {
                values: Rows::value_stream(projected_rows),
                next_cursor: input_response.next_cursor,
                prev_cursor: input_response.prev_cursor,
            },
        );

        Ok(())
    }
}

impl From<Project> for Action {
    fn from(value: Project) -> Self {
        Action::Project(value)
    }
}
