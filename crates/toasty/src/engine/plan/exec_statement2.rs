use crate::engine::plan::VarId;

use super::{stmt, Action};

#[derive(Debug)]
pub(crate) struct ExecStatement2 {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: Option<ExecStatementOutput>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,
}

#[derive(Debug)]
pub(crate) struct ExecStatementOutput {
    /// Databases always return rows as a vec of values. This specifies the type
    /// of each value.
    pub ty: Vec<stmt::Type>,
    pub var: VarId,
}

impl From<ExecStatement2> for Action {
    fn from(value: ExecStatement2) -> Self {
        Self::ExecStatement2(value)
    }
}
