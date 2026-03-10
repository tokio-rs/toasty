use crate::{stmt, Cursor, Executor, Load, Model, Result, Statement};

use std::future::Future;
use toasty_core::stmt::ValueStream;

/// Extension methods for [`Executor`].
///
/// Automatically implemented for every type that implements `Executor`.
/// These methods are generic over the model type, so they cannot be part of
/// the dyn-compatible `Executor` trait.
pub trait ExecutorExt: Executor {
    /// Execute a query, returning all matching records.
    fn all<M: Load>(&mut self, query: stmt::Select<M>) -> impl Future<Output = Result<Cursor<M>>> {
        async move {
            let records = self.exec(query.into()).await?;
            Ok(Cursor::new(records))
        }
    }

    /// Execute a query, returning the first matching record or `None`.
    fn first<M: Load>(
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
    fn get<M: Load>(&mut self, query: stmt::Select<M>) -> impl Future<Output = Result<M>> {
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
    fn delete<M>(&mut self, query: stmt::Select<M>) -> impl Future<Output = Result<()>> {
        async move {
            self.exec(query.delete().into()).await?;
            Ok(())
        }
    }

    /// Execute a statement, returning a raw value stream.
    fn exec<M>(&mut self, statement: Statement<M>) -> impl Future<Output = Result<ValueStream>> {
        async move {
            let untyped = statement.untyped;
            self.exec_untyped(untyped).await
        }
    }

    /// Execute a statement, expecting exactly one record.
    #[doc(hidden)]
    fn exec_one<M>(
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
            let mut cursor = Cursor::new(records);

            cursor.next().await.unwrap()
        }
    }
}

impl<T: Executor + ?Sized> ExecutorExt for T {}
