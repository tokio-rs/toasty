use crate::{
    Result,
    engine::exec::{Action, Exec, VarId},
};

/// Decrements a variable's use count without observing its value.
///
/// Emitted in an [`If`](super::If) else arm for each load the skipped `then`
/// arm would have performed on a variable produced outside the block, keeping
/// slot refcounts exact on both paths.
#[derive(Debug)]
pub(crate) struct Release {
    /// The variable to release.
    pub(crate) var: VarId,
}

impl Exec<'_> {
    pub(super) fn action_release(&mut self, action: &Release) -> Result<()> {
        self.vars.release(action.var);
        Ok(())
    }
}

impl From<Release> for Action {
    fn from(value: Release) -> Self {
        Action::Release(value)
    }
}
