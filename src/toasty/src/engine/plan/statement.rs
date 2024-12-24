use super::*;

#[derive(Debug)]
pub(crate) struct Statement {
    /// Where to get arguments for this action.
    pub input: Option<Input>,

    /// How to handle output
    pub output: Option<Output>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,
}

impl From<Statement> for Action {
    fn from(value: Statement) -> Self {
        Action::Statement(value)
    }
}
