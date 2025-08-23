use super::{eval, stmt, Action, Input, Output};

#[derive(Debug)]
pub(crate) struct ExecStatement {
    /// Where to get arguments for this action.
    pub input: Option<Input>,

    /// How to handle output
    pub output: Option<Output>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,

    /// HAX: this should be handled more generically, but for now, lets just get
    /// it working.
    pub conditional_update_with_no_returning: bool,

    /// Pagination configuration if this is a paginated query
    pub pagination: Option<Pagination>,
}

/// Pagination configuration for a query
#[derive(Debug)]
pub(crate) struct Pagination {
    /// Original limit before +1 transformation
    pub limit: u64,
    
    /// Function to extract cursor from a row
    /// Takes row as arg[0], returns cursor value(s)
    pub extract_cursor: eval::Func,
}

impl From<ExecStatement> for Action {
    fn from(value: ExecStatement) -> Self {
        Self::ExecStatement(value)
    }
}
