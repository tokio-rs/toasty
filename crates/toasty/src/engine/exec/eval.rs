use crate::{
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
    Result,
};
use toasty_core::driver::Rows;

#[derive(Debug)]
pub(crate) struct Eval {
    /// Input sources.
    pub(crate) inputs: Vec<VarId>,

    /// Output variable, where to store the result of the evaluation
    pub(crate) output: Output,

    /// How to evaluate
    pub(crate) eval: eval::Func,
}

impl Exec<'_> {
    pub(super) async fn action_eval(&mut self, action: &Eval) -> Result<()> {
        todo!("action={action:#?}");
    }
}

impl From<Eval> for Action {
    fn from(value: Eval) -> Self {
        Action::Eval(value)
    }
}
