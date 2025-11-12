use crate::{
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::driver::Rows;

impl Exec<'_> {
    pub(super) fn action_set_var(&mut self, action: &plan::SetVar) -> Result<()> {
        // Store the projected stream to the output variable
        self.vars.store(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(action.rows.clone()),
        );

        Ok(())
    }
}
