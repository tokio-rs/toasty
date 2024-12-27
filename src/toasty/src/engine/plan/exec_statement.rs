use super::*;

#[derive(Debug)]
pub(crate) struct ExecStatement {
    /// Where to get arguments for this action.
    pub input: Option<Input>,

    /// How to handle output
    pub output: Option<Output>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,
}

impl From<ExecStatement> for Action {
    fn from(value: ExecStatement) -> Self {
        Action::ExecStatement(value)
    }
}
