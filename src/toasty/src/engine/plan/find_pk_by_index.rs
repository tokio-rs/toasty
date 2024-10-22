use super::*;
use crate::{driver::*, schema::*};

/// Schema: `self` references [index-fields, input-fields] flattened
#[derive(Debug)]
pub(crate) struct FindPkByIndex<'stmt> {
    /// How to access input from the variable table.
    pub input: Vec<Input<'stmt>>,

    /// Where to store the output
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Index to use
    pub index: IndexId,

    /// Filter to apply to index
    pub filter: sql::Expr<'stmt>,
}

impl<'stmt> FindPkByIndex<'stmt> {
    pub(crate) fn apply(&self) -> Result<operation::FindPkByIndex<'stmt>> {
        Ok(operation::FindPkByIndex {
            table: self.table,
            index: self.index,
            // TODO: don't apply if not needed
            filter: self.filter.clone(),
        })
    }
}

impl<'stmt> From<FindPkByIndex<'stmt>> for Action<'stmt> {
    fn from(src: FindPkByIndex<'stmt>) -> Action<'stmt> {
        Action::FindPkByIndex(src)
    }
}
