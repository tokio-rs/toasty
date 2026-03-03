use crate::{stmt, Cursor, Model, Result, Statement, Transaction};

use std::sync::Arc;
use toasty_core::{async_trait, stmt::ValueStream, Schema};

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
    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<ValueStream>;

    /// Returns the schema associated with this executor.
    #[doc(hidden)]
    fn schema(&mut self) -> &Arc<Schema>;
}

/// Extension methods for [`Executor`].
///
/// Automatically implemented for every type that implements `Executor`.
/// These methods are generic over the model type, so they cannot be part of
/// the dyn-compatible `Executor` trait.
#[async_trait]
pub trait ExecutorExt: Executor {
    /// Execute a query, returning all matching records.
    async fn all<M: Model + Send>(&mut self, query: stmt::Select<M>) -> Result<Cursor<M>> {
        let records = self.exec(query.into()).await?;
        Ok(Cursor::new(self.schema().clone(), records))
    }

    /// Execute a query, returning the first matching record or `None`.
    async fn first<M: Model + Send>(&mut self, query: stmt::Select<M>) -> Result<Option<M>> {
        let mut res = self.all(query).await?;
        match res.next().await {
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    /// Execute a query, returning exactly one record or an error.
    async fn get<M: Model + Send>(&mut self, query: stmt::Select<M>) -> Result<M> {
        let mut res = self.all(query).await?;

        match res.next().await {
            Some(Ok(value)) => Ok(value),
            Some(Err(err)) => Err(err),
            None => Err(toasty_core::Error::record_not_found(
                "query returned no results",
            )),
        }
    }

    /// Delete all records matching the query.
    async fn delete<M: Model + Send>(&mut self, query: stmt::Select<M>) -> Result<()> {
        self.exec(query.delete()).await?;
        Ok(())
    }

    /// Execute a statement, returning a raw value stream.
    async fn exec<M: Model + Send>(&mut self, statement: Statement<M>) -> Result<ValueStream> {
        let untyped = statement.untyped;
        self.exec_untyped(untyped).await
    }

    /// Execute a statement, expecting exactly one record.
    #[doc(hidden)]
    async fn exec_one<M: Model + Send>(&mut self, statement: Statement<M>) -> Result<stmt::Value> {
        let mut res = self.exec(statement).await?;
        let Some(ret) = res.next().await else {
            return Err(toasty_core::Error::record_not_found(
                "statement returned no results",
            ));
        };
        let next = res.next().await;
        let None = next else {
            return Err(toasty_core::Error::invalid_record_count(
                "expected 1 record, found multiple",
            ));
        };

        ret
    }

    /// Execute an insert, returning the created model instance.
    #[doc(hidden)]
    async fn exec_insert_one<M: Model + Send>(&mut self, mut stmt: stmt::Insert<M>) -> Result<M> {
        // TODO: HAX
        stmt.untyped.source.single = false;

        let records = self.exec(stmt.into()).await?;
        let mut cursor = Cursor::new(self.schema().clone(), records);

        cursor.next().await.unwrap()
    }
}

impl<T: Executor + ?Sized> ExecutorExt for T {}
