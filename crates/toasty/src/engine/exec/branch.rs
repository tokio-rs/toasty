use crate::{
    Result,
    engine::{
        eval,
        exec::{Action, Exec, VarId},
    },
};

/// A conditionally executed block of actions — the exec program's first
/// control-flow construct.
///
/// The `then` arm holds pure actions whose outputs are only consumed when the
/// condition holds; mutations never appear in either arm. The `else` arm is
/// generated bookkeeping that keeps variable slots consistent when the block
/// is skipped.
#[derive(Debug)]
pub(crate) struct If {
    /// The condition deciding which arm runs.
    pub(crate) cond: Cond,

    /// Actions run when the condition holds.
    pub(crate) then: Vec<Action>,

    /// Runs when the condition is false: placeholder assignments for the
    /// `then` arm's escaping outputs and releases for its external input
    /// loads, so consumers outside the block never see an unset slot and use
    /// counts stay exact on both paths.
    pub(crate) r#else: Vec<Action>,
}

/// A condition an [`If`] action evaluates against the variable store.
#[derive(Debug)]
pub(crate) enum Cond {
    /// The variable holds a non-empty row list.
    NonEmpty(VarId),

    /// A boolean expression over the listed variables. Evaluation loads each
    /// input exactly once, on both arms, so the loads are part of the
    /// variables' use counts.
    Expr {
        /// Boolean expression evaluated against `inputs`.
        func: eval::Func,

        /// Input variables for the evaluation.
        inputs: Vec<VarId>,
    },
}

impl Exec<'_> {
    pub(super) async fn action_if(&mut self, action: &If) -> Result<()> {
        let pass = match &action.cond {
            // A non-consuming peek: buffers the condition variable's stream
            // in place, leaving its use count untouched.
            Cond::NonEmpty(var) => self.vars.peek_non_empty(*var).await?,
            Cond::Expr { func, inputs } => {
                let input = self.collect_input(inputs).await?;
                func.eval_bool(&self.engine.schema, &input)?
            }
        };

        let arm = if pass { &action.then } else { &action.r#else };

        for step in arm {
            self.exec_leaf_step(step).await?;
        }

        Ok(())
    }
}

impl From<If> for Action {
    fn from(value: If) -> Self {
        Action::If(value)
    }
}
