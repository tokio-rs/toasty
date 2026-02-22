use std::borrow::Cow;
use toasty_core::driver::transaction::{IsolationLevel, NestingTracker};

/// SQL generator for MySQL transactions. Isolation level is set via a
/// separate pre-statement; see [`start`] for the `(pre, begin)` tuple.
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

    /// Returns `(pre_stmt, begin_stmt)`.
    /// Execute `pre_stmt` first if `Some`, then `begin_stmt`.
    pub fn start(
        &mut self,
        isolation: Option<IsolationLevel>,
    ) -> (Option<String>, Cow<'static, str>) {
        if self.inner.depth() == 0 {
            let pre = isolation
                .map(|level| format!("SET TRANSACTION ISOLATION LEVEL {}", level.sql_name()));
            self.inner.begin();
            (pre, Cow::Borrowed("START TRANSACTION"))
        } else {
            (None, self.inner.savepoint())
        }
    }

    pub fn commit(&mut self) -> Cow<'static, str> {
        self.inner.commit()
    }

    pub fn rollback(&mut self) -> Cow<'static, str> {
        self.inner.rollback()
    }
}
