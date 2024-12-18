use super::*;
use crate::{driver::*, schema::*};

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

impl From<FindPkByIndex> for Action {
    fn from(src: FindPkByIndex) -> Action {
        Action::FindPkByIndex(src)
    }
}
