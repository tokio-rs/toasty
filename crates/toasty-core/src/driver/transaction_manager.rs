use std::borrow::Cow;

/// Manages transaction nesting depth and generates the correct SQL for
/// `BEGIN`/`SAVEPOINT`, `COMMIT`/`RELEASE SAVEPOINT`, and
/// `ROLLBACK`/`ROLLBACK TO SAVEPOINT` based on the current nesting level.
///
/// Each SQL driver embeds one of these (created via the appropriate factory
/// method) and calls `start`, `commit`, or `rollback` to obtain the SQL
/// string(s) to execute against the database.
#[derive(Debug)]
pub struct TransactionManager {
    depth: u32,
    begin_stmt: &'static str,
}

impl TransactionManager {
    fn with_begin(begin_stmt: &'static str) -> Self {
        Self {
            depth: 0,
            begin_stmt,
        }
    }

    /// Create a `TransactionManager` configured for SQLite (`BEGIN` / `COMMIT` / `ROLLBACK`).
    pub fn sqlite() -> Self {
        Self::with_begin("BEGIN")
    }

    /// Create a `TransactionManager` configured for MySQL (`START TRANSACTION` / `COMMIT` / `ROLLBACK`).
    pub fn mysql() -> Self {
        Self::with_begin("START TRANSACTION")
    }

    /// Create a `TransactionManager` configured for PostgreSQL (`BEGIN` / `COMMIT` / `ROLLBACK`).
    pub fn postgresql() -> Self {
        Self::with_begin("BEGIN")
    }

    /// Returns the SQL to begin a transaction or create a savepoint, and
    /// increments the nesting depth.
    pub fn start(&mut self) -> Cow<'static, str> {
        let sql = if self.depth == 0 {
            Cow::Borrowed(self.begin_stmt)
        } else {
            Cow::Owned(format!("SAVEPOINT sp_{}", self.depth))
        };
        self.depth += 1;
        sql
    }

    /// Returns the SQL to commit the current transaction or release a
    /// savepoint, and decrements the nesting depth.
    pub fn commit(&mut self) -> Cow<'static, str> {
        self.depth -= 1;
        if self.depth == 0 {
            Cow::Borrowed("COMMIT")
        } else {
            Cow::Owned(format!("RELEASE SAVEPOINT sp_{}", self.depth))
        }
    }

    /// Returns the SQL to roll back the current transaction or savepoint, and
    /// decrements the nesting depth.
    ///
    /// For nested transactions this is `ROLLBACK TO SAVEPOINT sp_N`. The
    /// savepoint is intentionally left in place — the outer `COMMIT` or
    /// `ROLLBACK` will clean it up, and re-entering a nested transaction at
    /// the same depth simply replaces it with a new `SAVEPOINT sp_N`.
    pub fn rollback(&mut self) -> Cow<'static, str> {
        self.depth -= 1;
        if self.depth == 0 {
            Cow::Borrowed("ROLLBACK")
        } else {
            Cow::Owned(format!("ROLLBACK TO SAVEPOINT sp_{}", self.depth))
        }
    }
}
