use super::*;

use crate::stmt;

#[derive(Debug)]
pub struct QuerySql<'stmt> {
    /// The SQL query to execute
    pub stmt: stmt::Statement<'stmt>,

    /// The type of each item in the returned stream
    ///
    /// If `None` then the statement has no output
    pub ty: Option<stmt::Type>,
}

impl<'stmt> From<QuerySql<'stmt>> for Operation<'stmt> {
    fn from(value: QuerySql<'stmt>) -> Operation<'stmt> {
        Operation::QuerySql(value)
    }
}
