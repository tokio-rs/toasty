use super::*;
use crate::{driver::*, schema::*};

/// Schema: `self` references [index-fields, input-fields] flattened
#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    /// How to access input from the variable table.
    pub input: Vec<Input>,

    /// Where to store the output
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Index to use
    pub index: IndexId,

    /// Filter to apply to index
    pub filter: stmt::Expr,
}

impl FindPkByIndex {
    pub(crate) fn apply(&self) -> Result<operation::FindPkByIndex> {
        Ok(operation::FindPkByIndex {
            table: self.table,
            index: self.index,
            // TODO: don't apply if not needed
            filter: self.filter.clone(),
        })
    }
}

impl From<FindPkByIndex> for Action {
    fn from(src: FindPkByIndex) -> Action {
        Action::FindPkByIndex(src)
    }
}
