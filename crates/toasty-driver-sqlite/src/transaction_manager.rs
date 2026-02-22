use std::borrow::Cow;
use toasty_core::driver::transaction::{IsolationLevel, NestingTracker};

/// SQL generator for SQLite transactions. Only `Serializable` and `None`
/// are accepted; other isolation levels return an error.
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

    /// Returns `BEGIN` or `SAVEPOINT sp_N`. Errors for unsupported isolation levels.
    pub fn start(
        &mut self,
        isolation: Option<IsolationLevel>,
    ) -> toasty_core::Result<Cow<'static, str>> {
        if self.inner.depth() == 0 {
            match isolation {
                None | Some(IsolationLevel::Serializable) => {}
                Some(_) => {
                    return Err(toasty_core::Error::unsupported_feature(
                        "SQLite only supports Serializable isolation",
                    ))
                }
            }
            self.inner.begin();
            Ok(Cow::Borrowed("BEGIN"))
        } else {
            Ok(self.inner.savepoint())
        }
    }

    pub fn commit(&mut self) -> Cow<'static, str> {
        self.inner.commit()
    }

    pub fn rollback(&mut self) -> Cow<'static, str> {
        self.inner.rollback()
    }
}
