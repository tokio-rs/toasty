use crate::{schema::Load, Result, Statement, Transaction};

use std::sync::Arc;
use toasty_core::{async_trait, stmt::Value, Schema};

/// Anything that can execute queries — `Db` or `Transaction`.
///
/// This trait is dyn-compatible. Generic convenience methods live on
/// [`ExecutorExt`], which is blanket-implemented for all `Executor` types.
#[async_trait]
pub trait Executor: Send + Sync {
    /// Starts a (potentially nested) transaction.
    async fn transaction(&mut self) -> Result<Transaction<'_>>;

    /// Execute an untyped statement, returning a raw value stream.
    #[doc(hidden)]
    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<Value>;

    /// Returns the schema associated with this executor.
    #[doc(hidden)]
    fn schema(&mut self) -> &Arc<Schema>;
}

impl dyn Executor + '_ {
    pub async fn exec<T: Load>(&mut self, stmt: Statement<T>) -> Result<T::Output> {
        let res = self.exec_untyped(stmt.untyped).await?;
        T::load(res)
    }
}
