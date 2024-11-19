use super::*;

use crate::{schema::TableId, stmt};

#[derive(Debug)]
pub struct UpdateByKey {
    /// Which table to update
    pub table: TableId,

    /// Which keys to update
    pub keys: Vec<stmt::Value<'static>>,

    /// How to update the table
    pub assignments: stmt::Assignments<'static>,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr<'static>>,

    /// Any conditions that must hold to apply the update
    pub condition: Option<stmt::Expr<'static>>,

    /// If true, then the driver should return a record for each instance of the
    /// model that was updated.
    pub returning: bool,
}

impl From<UpdateByKey> for Operation {
    fn from(value: UpdateByKey) -> Operation {
        Operation::UpdateByKey(value)
    }
}
