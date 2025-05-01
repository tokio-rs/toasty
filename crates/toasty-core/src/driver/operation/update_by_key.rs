use super::*;

use crate::{schema::db::TableId, stmt};

#[derive(Debug)]
pub struct UpdateByKey {
    /// Which table to update
    pub table: TableId,

    /// Which keys to update
    pub keys: Vec<stmt::Value>,

    /// How to update the table
    pub assignments: stmt::Assignments,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr>,

    /// Any conditions that must hold to apply the update
    pub condition: Option<stmt::Expr>,

    /// If true, then the driver should return a record for each instance of the
    /// model that was updated.
    pub returning: bool,
}

impl From<UpdateByKey> for Operation {
    fn from(value: UpdateByKey) -> Self {
        Self::UpdateByKey(value)
    }
}
