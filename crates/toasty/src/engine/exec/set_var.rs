use toasty_core::driver::Rows;

use crate::{
    engine::{exec::Exec, plan},
    Result,
};

impl Exec<'_> {
    pub(super) fn action_set_var2(&mut self, action: &plan::SetVar2) -> Result<()> {
        // Store the projected stream to the output variable
        self.vars.store_counted(
            action.output.var,
            action.output.num_uses,
            Rows::value_stream(action.rows.clone()),
        );

        Ok(())
    }
}
