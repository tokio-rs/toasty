use super::*;

use crate::{schema::TableId, stmt};

#[derive(Debug)]
pub struct UpdateByKey<'stmt> {
    /// Which table to update
    pub table: TableId,

    /// Which keys to update
    pub keys: Vec<stmt::Value<'stmt>>,

    /// How to update the table
    pub assignments: Vec<sql::Assignment<'stmt>>,

    /// Only update keys that match the filter
    pub filter: Option<sql::Expr<'stmt>>,

    /// Any conditions that must hold to apply the update
    pub condition: Option<sql::Expr<'stmt>>,

    /// If true, then the driver should return a record for each instance of the
    /// model that was updated.
    pub returning: bool,
}

impl<'stmt> From<UpdateByKey<'stmt>> for Operation<'stmt> {
    fn from(value: UpdateByKey<'stmt>) -> Operation<'stmt> {
        Operation::UpdateByKey(value)
    }
}
