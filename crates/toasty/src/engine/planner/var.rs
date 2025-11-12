use toasty_core::stmt;

use crate::engine::exec::VarId;

/// Tracks available slots to store record streams in. These slots are used to
/// pass record streams from action outputs into the next input.
#[derive(Debug, Default)]
pub(crate) struct VarTable {
    /// Variable slots used during plan execution
    vars: Vec<stmt::Type>,
}

impl VarTable {
    #[track_caller]
    pub fn register_var(&mut self, ty: stmt::Type) -> VarId {
        debug_assert!(ty.is_list() || ty.is_unit(), "{ty:#?}");
        // Register a new slot
        let ret = self.vars.len();
        self.vars.push(ty);
        VarId(ret)
    }

    pub(crate) fn into_vec(self) -> Vec<stmt::Type> {
        self.vars
    }
}
