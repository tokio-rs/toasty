use crate::engine::plan::{Output2, VarId};

use super::{stmt, Action};

#[derive(Debug)]
pub(crate) struct ExecStatement2 {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: ExecStatementOutput,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,

    /// When true, the statement is a conditional update without any returning.
    pub conditional_update_with_no_returning: bool,
}

#[derive(Debug)]
pub(crate) struct ExecStatementOutput {
    /// Databases always return rows as a vec of values. This specifies the type
    /// of each value.
    pub ty: Option<Vec<stmt::Type>>,
    pub output: Output2,
}

impl From<ExecStatement2> for Action {
    fn from(value: ExecStatement2) -> Self {
        Self::ExecStatement2(value)
    }
}
