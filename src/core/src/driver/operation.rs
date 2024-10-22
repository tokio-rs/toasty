mod delete_by_key;
pub use delete_by_key::DeleteByKey;

mod find_pk_by_index;
pub use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub use get_by_key::GetByKey;

mod query_pk;
pub use query_pk::QueryPk;

mod query_sql;
pub use query_sql::QuerySql;

mod update_by_key;
pub use update_by_key::UpdateByKey;

use super::*;

#[derive(Debug)]
pub enum Operation<'stmt> {
    /// Create a new record. This will always be a `sql::Insert`
    Insert(sql::Statement<'stmt>),

    /// Delete records identified by the given keys.
    DeleteByKey(DeleteByKey<'stmt>),

    /// Find by index
    FindPkByIndex(FindPkByIndex<'stmt>),

    /// Get one or more records by the primary key
    GetByKey(GetByKey<'stmt>),

    /// Query the table, filtering by the primary key
    QueryPk(QueryPk<'stmt>),

    /// Execute a SQL query
    QuerySql(QuerySql<'stmt>),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey<'stmt>),
}
