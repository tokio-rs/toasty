use super::*;

use crate::stmt;

#[derive(Debug)]
pub struct QuerySql {
    /// The SQL query to execute
    pub stmt: stmt::Statement,

    /// The return type
    pub ret: Option<Vec<stmt::Type>>,
}

impl From<QuerySql> for Operation {
    fn from(value: QuerySql) -> Self {
        Self::QuerySql(value)
    }
}
