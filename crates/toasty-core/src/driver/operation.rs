mod delete_by_key;
pub use delete_by_key::DeleteByKey;

mod find_pk_by_index;
pub use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub use get_by_key::GetByKey;

mod insert;
pub use insert::Insert;

mod query_pk;
pub use query_pk::QueryPk;

mod query_sql;
pub use query_sql::QuerySql;

mod transaction;
pub use transaction::Transaction;

mod update_by_key;
pub use update_by_key::UpdateByKey;

use super::*;

#[derive(Debug)]
pub enum Operation {
    /// Create a new record. This will always be a lowered `stmt::Insert`
    Insert(Insert),

    /// Delete records identified by the given keys.
    DeleteByKey(DeleteByKey),

    /// Find by index
    FindPkByIndex(FindPkByIndex),

    /// Get one or more records by the primary key
    GetByKey(GetByKey),

    /// Query the table, filtering by the primary key
    QueryPk(QueryPk),

    /// Execute a SQL query
    QuerySql(QuerySql),

    /// Execute a transaction lifecycle op
    Transaction(Transaction),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey),
}
