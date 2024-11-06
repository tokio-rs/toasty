use super::*;

#[derive(Debug)]
pub(crate) struct QuerySql<'stmt> {
    /// Where to get arguments for this action.
    pub input: Vec<Input<'stmt>>,

    /// How to handle output
    pub output: Option<QuerySqlOutput<'stmt>>,

    /// The query to execute. This may require input to generate the query.
    pub stmt: stmt::Statement<'stmt>,
}

#[derive(Debug)]
pub(crate) struct QuerySqlOutput<'stmt> {
    /// Variable to store the output in
    pub var: plan::VarId,

    /// How to project the output returned by the driver
    pub project: eval::Expr<'stmt>,
}

impl<'stmt> From<QuerySql<'stmt>> for Action<'stmt> {
    fn from(value: QuerySql<'stmt>) -> Self {
        Action::QuerySql(value)
    }
}
