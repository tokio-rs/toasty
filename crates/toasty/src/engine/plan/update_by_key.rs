use super::*;

use crate::schema::db::TableId;

#[derive(Debug, Clone)]
pub(crate) struct UpdateByKey {
    /// If specified, use the input to generate the list of keys to update
    pub input: Option<Input>,

    /// Where to store the result of the update
    pub output: Option<Output>,

    /// Which table to update
    pub table: TableId,

    /// Which key(s) to update
    pub keys: eval::Func,

    /// Assignments
    pub assignments: stmt::Assignments,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr>,

    /// Fail the update if the condition is not met
    pub condition: Option<stmt::Expr>,
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Self {
        Self::UpdateByKey(src)
    }
}
