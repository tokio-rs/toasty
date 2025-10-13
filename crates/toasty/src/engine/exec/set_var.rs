use crate::{
    engine::{exec::Exec, plan},
    Result,
};
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) fn action_set_var2(&mut self, action: &plan::SetVar2) -> Result<()> {
        // Store the projected stream to the output variable
        self.vars.store_counted(
            action.output.var,
            action.output.num_uses,
            ValueStream::from_vec(action.value.clone()),
        );

        Ok(())
    }
}
