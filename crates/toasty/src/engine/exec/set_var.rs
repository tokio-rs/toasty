use crate::{
    engine::exec::{Action, Exec, Output, VarId},
    Result,
};
use toasty_core::{driver::Rows, stmt};

#[derive(Debug)]
pub(crate) struct SetVar {
    pub(crate) source: VarSource,
    pub(crate) output: Output,
}

/// Where SetVar gets its value from.
#[derive(Debug)]
pub(crate) enum VarSource {
    /// A constant value.
    Value(stmt::Value),
    /// Copy from another variable (move the Rows directly).
    Var(VarId),
    /// A count (for Unit-typed vars, e.g., mutation results).
    Count(u64),
}

impl Exec<'_> {
    pub(super) async fn action_set_var(&mut self, action: &SetVar) -> Result<()> {
        match &action.source {
            VarSource::Value(value) => {
                self.vars.store(
                    action.output.var,
                    action.output.num_uses,
                    Rows::Value(value.clone()),
                );
            }
            VarSource::Var(src_var) => {
                let rows = self.vars.load(*src_var).await?;
                self.vars
                    .store(action.output.var, action.output.num_uses, rows);
            }
            VarSource::Count(count) => {
                self.vars.store(
                    action.output.var,
                    action.output.num_uses,
                    Rows::Count(*count),
                );
            }
        }

        Ok(())
    }
}

impl From<SetVar> for Action {
    fn from(value: SetVar) -> Self {
        Self::SetVar(value)
    }
}
