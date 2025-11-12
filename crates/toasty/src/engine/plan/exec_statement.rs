use toasty_core::stmt;

use crate::engine::plan::{Action, Output, VarId};

#[derive(Debug)]
pub(crate) struct ExecStatement {
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
    pub output: Output,
}

impl From<ExecStatement> for Action {
    fn from(value: ExecStatement) -> Self {
        Self::ExecStatement(value)
    }
}
