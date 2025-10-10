use crate::engine::plan::VarId;

use super::{stmt, Action, Input, Output};
use toasty_core::schema::db::{IndexId, TableId};

/// Schema: `self` references [index-fields, input-fields] flattened
#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    /// How to access input from the variable table.
    pub input: Option<Input>,

    /// Where to store the output
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Index to use
    pub index: IndexId,

    /// Filter to apply to index
    pub filter: stmt::Expr,
}

/// Schema: `self` references [index-fields, input-fields] flattened
#[derive(Debug)]
pub(crate) struct FindPkByIndex2 {
    /// How to access input from the variable table.
    pub input: Vec<VarId>,

    /// Where to store the output
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Index to use
    pub index: IndexId,

    /// Filter to apply to index
    pub filter: stmt::Expr,
}

impl From<FindPkByIndex> for Action {
    fn from(src: FindPkByIndex) -> Self {
        Self::FindPkByIndex(src)
    }
}

impl From<FindPkByIndex2> for Action {
    fn from(src: FindPkByIndex2) -> Self {
        Self::FindPkByIndex2(src)
    }
}
