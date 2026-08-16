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
/// condition holds; mutations never appear in it. Skipping the arm is pure
/// bookkeeping, declared as data: `skipped_inputs` and `empty_outputs` keep
/// variable slots consistent so consumers outside the block never see an
/// unset slot and use counts stay exact on both paths.
#[derive(Debug)]
pub(crate) struct If {
    /// The condition deciding whether the `then` arm runs.
    pub(crate) cond: Cond,

    /// Actions run when the condition holds.
    pub(crate) then: Vec<Action>,

    /// Variables produced outside the block that the `then` arm would have
    /// loaded — one entry per declined load. Released when the arm is
    /// skipped.
    pub(crate) skipped_inputs: Vec<VarId>,

    /// The `then` arm's escaping outputs, each with its external use count.
    /// When the arm is skipped, each is assigned the empty value of its
    /// declared type.
    pub(crate) empty_outputs: Vec<(VarId, usize)>,
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

        if pass {
            for step in &action.then {
                self.exec_leaf_step(step).await?;
            }
        } else {
            for &var in &action.skipped_inputs {
                self.vars.release(var);
            }
            for &(var, num_uses) in &action.empty_outputs {
                self.vars.store_empty(var, num_uses);
            }
        }

        Ok(())
    }
}

impl From<If> for Action {
    fn from(value: If) -> Self {
        Action::If(value)
    }
}
