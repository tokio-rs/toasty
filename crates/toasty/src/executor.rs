use crate::{stmt, Cursor, Model, Result, Statement, Transaction};

use std::future::Future;
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
pub trait ExecutorExt: Executor {
    /// Execute a query, returning all matching records.
    fn all<M: Model>(&mut self, query: stmt::Select<M>) -> impl Future<Output = Result<Cursor<M>>> {
        async move {
            let records = self.exec(query.into()).await?;
            Ok(Cursor::new(self.schema().clone(), records))
        }
    }

    /// Execute a query, returning the first matching record or `None`.
    fn first<M: Model>(
        &mut self,
        query: stmt::Select<M>,
    ) -> impl Future<Output = Result<Option<M>>> {
        async move {
            let mut res = self.all(query).await?;
            match res.next().await {
                Some(Ok(value)) => Ok(Some(value)),
                Some(Err(err)) => Err(err),
                None => Ok(None),
            }
        }
    }

    /// Execute a query, returning exactly one record or an error.
    fn get<M: Model>(&mut self, query: stmt::Select<M>) -> impl Future<Output = Result<M>> {
        async move {
            let mut res = self.all(query).await?;

            match res.next().await {
                Some(Ok(value)) => Ok(value),
                Some(Err(err)) => Err(err),
                None => Err(toasty_core::Error::record_not_found(
                    "query returned no results",
                )),
            }
        }
    }

    /// Delete all records matching the query.
    fn delete<M: Model>(&mut self, query: stmt::Select<M>) -> impl Future<Output = Result<()>> {
        async move {
            self.exec(query.delete()).await?;
            Ok(())
        }
    }

    /// Execute a statement, returning a raw value stream.
    fn exec<M: Model>(
        &mut self,
        statement: Statement<M>,
    ) -> impl Future<Output = Result<ValueStream>> {
        async move {
            let untyped = statement.untyped;
            self.exec_untyped(untyped).await
        }
    }

    /// Execute a statement, expecting exactly one record.
    #[doc(hidden)]
    fn exec_one<M: Model>(
        &mut self,
        statement: Statement<M>,
    ) -> impl Future<Output = Result<stmt::Value>> {
        async move {
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
    }

    /// Execute an insert, returning the created model instance.
    #[doc(hidden)]
    fn exec_insert_one<M: Model>(
        &mut self,
        mut stmt: stmt::Insert<M>,
    ) -> impl Future<Output = Result<M>> {
        async move {
            // TODO: HAX
            stmt.untyped.source.single = false;

            let records = self.exec(stmt.into()).await?;
            let mut cursor = Cursor::new(self.schema().clone(), records);

            cursor.next().await.unwrap()
        }
    }
}

impl<T: Executor + ?Sized> ExecutorExt for T {}
