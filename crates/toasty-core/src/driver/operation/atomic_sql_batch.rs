use super::QuerySql;

/// An ordered group of generated SQL statements that must execute atomically.
///
/// Drivers must either execute the complete group as one backend atomic unit
/// or return an error without executing any statement. Raw SQL and non-SQL
/// operations are deliberately excluded.
#[derive(Debug, Clone)]
pub struct AtomicSqlBatch {
    /// Generated SQL operations in execution order.
    pub operations: Vec<QuerySql>,
}

impl AtomicSqlBatch {
    /// Creates an atomic batch from ordered generated SQL operations.
    pub fn new(operations: Vec<QuerySql>) -> Self {
        Self { operations }
    }
}
