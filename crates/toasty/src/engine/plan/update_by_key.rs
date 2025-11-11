use crate::engine::plan::{Action, Output, VarId};
use toasty_core::{schema::db::TableId, stmt};

#[derive(Debug, Clone)]
pub(crate) struct UpdateByKey {
    /// If specified, use the input to generate the list of keys to update
    pub input: VarId,

    /// Where to store the result of the update
    pub output: Output,

    /// Which table to update
    pub table: TableId,

    /// Assignments
    pub assignments: stmt::Assignments,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr>,

    /// Fail the update if the condition is not met
    pub condition: Option<stmt::Expr>,

    /// When `true` return the record being updated *after* the update. When
    /// `false`, just return the count of updated rows.
    pub returning: bool,
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Self {
        Self::UpdateByKey(src)
    }
}
