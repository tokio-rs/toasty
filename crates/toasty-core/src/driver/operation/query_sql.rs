use super::Operation;

use crate::stmt;

#[derive(Debug, Clone)]
pub struct QuerySql {
    /// The SQL query to execute
    pub stmt: stmt::Statement,

    /// The return type
    pub ret: Option<Vec<stmt::Type>>,

    /// **TEMPORARY HACK**: MySQL-specific workaround for RETURNING from INSERT.
    ///
    /// When set, indicates this query should be preceded by fetching LAST_INSERT_ID()
    /// to simulate RETURNING behavior for the specified number of inserted rows.
    /// The query will return a list of rows, each with a single column containing
    /// the auto-increment ID.
    ///
    /// Non-MySQL drivers should assert this is None.
    pub last_insert_id_hack: Option<u64>,
}

impl From<QuerySql> for Operation {
    fn from(value: QuerySql) -> Self {
        Self::QuerySql(value)
    }
}
