//! Database operations dispatched to drivers.
//!
//! An [`Operation`] is the unit of work sent to [`Connection::exec`](super::Connection::exec).
//! The query engine compiles user queries into one or more `Operation` values.
//! SQL drivers handle [`QuerySql`] and [`Insert`]; key-value drivers handle
//! [`GetByKey`], [`QueryPk`], [`DeleteByKey`], [`FindPkByIndex`], [`UpdateByKey`],
//! and (when [`Capability::scan`](super::Capability::scan) is `true`) [`Scan`].
//! Both driver types handle [`Transaction`] operations.

mod delete_by_key;
pub use delete_by_key::DeleteByKey;

mod find_pk_by_index;
pub use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub use get_by_key::GetByKey;

mod insert;
pub use insert::Insert;

mod query_pk;
pub use query_pk::{QueryPk, QueryPkLimit};

mod query_sql;
pub use query_sql::QuerySql;

mod scan;
pub use scan::Scan;

mod transaction;
pub use transaction::{IsolationLevel, Transaction};

mod typed_value;
pub use typed_value::TypedValue;

mod update_by_key;
pub use update_by_key::UpdateByKey;

/// A single database operation to be executed by a driver.
///
/// Each variant maps to one logical database action. The query planner selects
/// variants based on the driver's [`Capability`](super::Capability): SQL
/// drivers receive [`QuerySql`](Self::QuerySql) and [`Insert`](Self::Insert),
/// while key-value drivers receive [`GetByKey`](Self::GetByKey),
/// [`QueryPk`](Self::QueryPk), etc.
///
/// All operation types implement `From<T> for Operation`, so they can be
/// converted with `.into()`.
///
/// # Examples
///
/// ```
/// use toasty_core::driver::operation::{Operation, Transaction};
///
/// let op: Operation = Transaction::start().into();
/// assert!(!op.is_transaction_commit());
/// ```
#[derive(Debug, Clone)]
pub enum Operation {
    /// Insert a new record. Contains a lowered [`stmt::Insert`](crate::stmt::Insert) statement.
    Insert(Insert),

    /// Delete one or more records identified by primary key.
    DeleteByKey(DeleteByKey),

    /// Look up primary keys via a secondary index.
    FindPkByIndex(FindPkByIndex),

    /// Fetch one or more records by exact primary key match.
    GetByKey(GetByKey),

    /// Query a table with a primary key filter, optional secondary filtering,
    /// ordering, and pagination.
    QueryPk(QueryPk),

    /// Execute a raw SQL statement. Only sent to SQL-capable drivers.
    QuerySql(QuerySql),

    /// A transaction lifecycle operation (begin, commit, rollback, savepoint).
    Transaction(Transaction),

    /// Update one or more records identified by primary key.
    UpdateByKey(UpdateByKey),

    /// Full-table scan with optional filter and pagination.
    ///
    /// Only sent to drivers with [`Capability::scan`](super::Capability::scan) set to `true`.
    Scan(Scan),
}

impl Operation {
    /// Returns the operation variant name for logging.
    pub fn name(&self) -> &str {
        match self {
            Operation::Insert(_) => "insert",
            Operation::DeleteByKey(_) => "delete_by_key",
            Operation::FindPkByIndex(_) => "find_pk_by_index",
            Operation::GetByKey(_) => "get_by_key",
            Operation::QueryPk(_) => "query_pk",
            Operation::QuerySql(_) => "query_sql",
            Operation::Transaction(_) => "transaction",
            Operation::UpdateByKey(_) => "update_by_key",
            Operation::Scan(_) => "scan",
        }
    }
}
