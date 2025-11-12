use crate::{
    engine::exec::{Action, Exec, Output},
    Result,
};
use toasty_core::{driver::Rows, stmt};

#[derive(Debug)]
pub(crate) struct SetVar {
    pub rows: Vec<stmt::Value>,
    pub output: Output,
}

impl Exec<'_> {
    pub(super) fn action_set_var(&mut self, action: &SetVar) -> Result<()> {
        // Store the projected stream to the output variable
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(action.rows.clone()),
        );

        Ok(())
    }
}

impl From<SetVar> for Action {
    fn from(value: SetVar) -> Self {
        Self::SetVar(value)
    }
}
