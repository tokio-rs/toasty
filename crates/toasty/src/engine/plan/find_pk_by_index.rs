use crate::engine::plan::{Action, Output2, VarId};
use toasty_core::{
    schema::db::{IndexId, TableId},
    stmt,
};

/// Schema: `self` references [index-fields, input-fields] flattened
#[derive(Debug)]
pub(crate) struct FindPkByIndex2 {
    /// How to access input from the variable table.
    pub input: Vec<VarId>,

    /// Where to store the output
    pub output: Output2,

    /// Table to query
    pub table: TableId,

    /// Index to use
    pub index: IndexId,

    /// Filter to apply to index
    pub filter: stmt::Expr,
}

impl From<FindPkByIndex2> for Action {
    fn from(src: FindPkByIndex2) -> Self {
        Self::FindPkByIndex2(src)
    }
}
