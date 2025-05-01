use super::*;

use crate::stmt;

#[derive(Debug)]
pub struct Insert {
    pub stmt: stmt::Statement,

    /// The return type
    pub ret: Option<Vec<stmt::Type>>,
}

impl From<Insert> for Operation {
    fn from(value: Insert) -> Self {
        Self::Insert(value)
    }
}
