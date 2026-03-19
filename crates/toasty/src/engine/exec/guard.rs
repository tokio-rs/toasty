use crate::{
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
    Result,
};
use toasty_core::{driver::Rows, stmt::ValueStream};

/// Gates a data stream with a boolean condition evaluated against separate
/// inputs. When the guard is `false`, an empty stream is produced.
#[derive(Debug, Clone)]
pub(crate) struct Guard {
    /// The data input to conditionally pass through.
    pub input: VarId,

    /// Input variables for guard evaluation.
    pub guard_inputs: Vec<VarId>,

    /// Where to store the output.
    pub output: Output,

    /// Boolean expression evaluated against `guard_inputs`.
    pub guard: eval::Func,
}

impl Exec<'_> {
    pub(super) async fn action_guard(&mut self, action: &Guard) -> Result<()> {
        // Evaluate the guard expression against its inputs.
        let mut inputs = Vec::with_capacity(action.guard_inputs.len());
        for var_id in &action.guard_inputs {
            let data = self.vars.load(*var_id).await?.collect_as_value().await?;
            inputs.push(data);
        }

        let pass = action.guard.eval_bool(&inputs)?;

        let res = if pass {
            // Guard passed — forward the input unchanged.
            self.vars.load(action.input).await?
        } else {
            // Guard failed — produce an empty stream.
            Rows::value_stream(ValueStream::default())
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}

impl From<Guard> for Action {
    fn from(value: Guard) -> Self {
        Action::Guard(value)
    }
}
