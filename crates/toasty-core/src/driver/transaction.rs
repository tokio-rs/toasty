use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl IsolationLevel {
    /// Returns the ANSI SQL name, usable in PostgreSQL and MySQL.
    pub fn sql_name(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }
}

/// Tracks nesting depth and generates SAVEPOINT/COMMIT/ROLLBACK SQL.
/// Drivers call `begin()` after their dialect-specific outer BEGIN, then
/// delegate `savepoint`, `commit`, and `rollback` here.
#[derive(Debug)]
pub struct NestingTracker {
    depth: u32,
}

impl Default for NestingTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl NestingTracker {
    pub fn new() -> Self {
        Self { depth: 0 }
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Increment depth after the outer BEGIN.
    pub fn begin(&mut self) {
        self.depth += 1;
    }

    /// Returns `SAVEPOINT sp_N` and increments depth.
    pub fn savepoint(&mut self) -> Cow<'static, str> {
        let sql = Cow::Owned(format!("SAVEPOINT sp_{}", self.depth));
        self.depth += 1;
        sql
    }

    /// Returns `COMMIT` or `RELEASE SAVEPOINT sp_N` and decrements depth.
    pub fn commit(&mut self) -> Cow<'static, str> {
        self.depth -= 1;
        if self.depth == 0 {
            Cow::Borrowed("COMMIT")
        } else {
            Cow::Owned(format!("RELEASE SAVEPOINT sp_{}", self.depth))
        }
    }

    /// Returns `ROLLBACK` or `ROLLBACK TO SAVEPOINT sp_N` and decrements depth.
    pub fn rollback(&mut self) -> Cow<'static, str> {
        self.depth -= 1;
        if self.depth == 0 {
            Cow::Borrowed("ROLLBACK")
        } else {
            Cow::Owned(format!("ROLLBACK TO SAVEPOINT sp_{}", self.depth))
        }
    }
}
