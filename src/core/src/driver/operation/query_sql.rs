use super::*;

use crate::stmt;

#[derive(Debug)]
pub struct QuerySql {
    /// The SQL query to execute
    pub stmt: stmt::Statement<'static>,
}

impl From<QuerySql> for Operation {
    fn from(value: QuerySql) -> Operation {
        Operation::QuerySql(value)
    }
}
