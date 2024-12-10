use super::*;

#[derive(Debug)]
pub(crate) struct QuerySql {
    /// Where to get arguments for this action.
    pub input: Option<Input>,

    /// How to handle output
    pub output: Option<QuerySqlOutput>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement,
}

#[derive(Debug)]
pub(crate) struct QuerySqlOutput {
    /// Variable to store the output in
    pub var: plan::VarId,

    /// How to project the output returned by the driver
    pub project: eval::Func,
}

impl From<QuerySql> for Action {
    fn from(value: QuerySql) -> Self {
        Action::QuerySql(value)
    }
}
