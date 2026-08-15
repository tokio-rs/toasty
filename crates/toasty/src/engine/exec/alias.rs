use crate::{
    Result,
    engine::exec::{Action, Exec, Output, VarId},
};

/// Passes a variable's value through to another slot unchanged.
///
/// With a sole consumer this is a move — the response (stream included)
/// relocates without buffering; a shared input pays the same duplication any
/// multi-consumer variable does.
#[derive(Debug)]
pub(crate) struct Alias {
    /// The variable to pass through.
    pub input: VarId,

    /// Where to store the value.
    pub output: Output,
}

impl Exec<'_> {
    pub(super) async fn action_alias(&mut self, action: &Alias) -> Result<()> {
        let response = self.vars.load(action.input).await?;
        self.vars
            .store(action.output.var, action.output.num_uses, response);
        Ok(())
    }
}

impl From<Alias> for Action {
    fn from(value: Alias) -> Self {
        Action::Alias(value)
    }
}
