use super::*;

/// Tracks available slots to store record streams in. These slots are used to
/// pass record streams from action outputs into the next input.
#[derive(Debug, Default)]
pub(crate) struct VarTable {
    /// Variable slots used during plan execution
    vars: Vec<stmt::Type>,
}

impl VarTable {
    pub fn register_var(&mut self, ty: stmt::Type) -> plan::VarId {
        // Register a new slot
        let ret = self.vars.len();
        self.vars.push(ty);
        plan::VarId(ret)
    }
}
