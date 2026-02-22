use std::borrow::Cow;
use toasty_core::driver::transaction::{IsolationLevel, NestingTracker};

/// SQL generator for PostgreSQL transactions. Isolation level is embedded
/// directly in the `BEGIN` statement.
#[derive(Debug)]
pub(crate) struct TransactionManager {
    inner: NestingTracker,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            inner: NestingTracker::new(),
        }
    }

    /// Returns `BEGIN [ISOLATION LEVEL X]` or `SAVEPOINT sp_N`.
    pub fn start(&mut self, isolation: Option<IsolationLevel>) -> Cow<'static, str> {
        if self.inner.depth() == 0 {
            self.inner.begin();
            match isolation {
                None => Cow::Borrowed("BEGIN"),
                Some(level) => Cow::Owned(format!("BEGIN ISOLATION LEVEL {}", level.sql_name())),
            }
        } else {
            self.inner.savepoint()
        }
    }

    pub fn commit(&mut self) -> Cow<'static, str> {
        self.inner.commit()
    }

    pub fn rollback(&mut self) -> Cow<'static, str> {
        self.inner.rollback()
    }
}
