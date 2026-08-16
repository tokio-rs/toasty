use crate::{
    Result,
    engine::exec::{Action, Exec, Output, VarId},
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt,
};

#[derive(Debug)]
pub(crate) struct Repeat {
    /// The variable whose cardinality drives the repetition.
    pub(crate) input: VarId,

    /// Where to store the output
    pub(crate) output: Output,

    /// The value produced once per input row.
    pub(crate) value: stmt::Value,
}

impl Exec<'_> {
    pub(super) async fn action_repeat(&mut self, action: &Repeat) -> Result<()> {
        let input_response = self.vars.load(action.input).await?;

        // Only the input's cardinality is observed; a count-only driver
        // response works the same as one that returned rows.
        let count = match input_response.values {
            Rows::Count(count) => count as usize,
            Rows::Value(stmt::Value::List(items)) => items.len(),
            Rows::Value(value) => todo!("value={value:#?}"),
            Rows::Stream(mut value_stream) => {
                let mut count = 0;
                while let Some(res) = value_stream.next().await {
                    res?;
                    count += 1;
                }
                count
            }
        };

        let rows = vec![action.value.clone(); count];

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse::from_rows(Rows::value_stream(rows)),
        );

        Ok(())
    }
}

impl From<Repeat> for Action {
    fn from(value: Repeat) -> Self {
        Action::Repeat(value)
    }
}
