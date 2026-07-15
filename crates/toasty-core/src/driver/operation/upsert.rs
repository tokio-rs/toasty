use super::{Operation, TypedValue};
use crate::stmt;

/// Executes a lowered single-row upsert on a database driver.
#[derive(Debug, Clone)]
pub struct Upsert {
    /// The lowered insert statement carrying its conflict clause.
    pub stmt: stmt::Insert,

    /// Typed SQL bind parameters extracted from the statement.
    pub params: Vec<TypedValue>,

    /// Types of the columns returned by the operation.
    pub ret: Option<Vec<stmt::Type>>,
}

impl From<Upsert> for Operation {
    fn from(value: Upsert) -> Self {
        Self::Upsert(value)
    }
}
