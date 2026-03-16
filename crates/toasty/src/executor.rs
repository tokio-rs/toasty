use crate::{stmt, Load, Model, Result, Statement, Transaction};

use std::sync::Arc;
use toasty_core::{async_trait, stmt::ValueStream, Schema};

/// Anything that can execute queries — `Db` or `Transaction`.
///
/// This trait is dyn-compatible. Generic convenience methods are available as
/// inherent methods on `dyn Executor`.
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

impl dyn Executor + '_ {
    /// Execute a query, returning all matching records.
    pub async fn all<M: Load<Output = M>>(&mut self, query: stmt::Query<M>) -> Result<Vec<M>> {
        let mut records = self.exec(query.into()).await?;
        let mut result = Vec::new();
        while let Some(value) = records.next().await {
            result.push(M::load(value?)?);
        }
        Ok(result)
    }

    /// Execute a query, returning the first matching record or `None`.
    pub async fn first<M: Load<Output = M>>(&mut self, query: stmt::Query<M>) -> Result<Option<M>> {
        let mut records = self.exec(query.into()).await?;
        match records.next().await {
            Some(Ok(value)) => Ok(Some(M::load(value)?)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    /// Execute a query, returning exactly one record or an error.
    pub async fn get<M: Load<Output = M>>(&mut self, query: stmt::Query<M>) -> Result<M> {
        let mut records = self.exec(query.into()).await?;

        match records.next().await {
            Some(Ok(value)) => Ok(M::load(value)?),
            Some(Err(err)) => Err(err),
            None => Err(toasty_core::Error::record_not_found(
                "query returned no results",
            )),
        }
    }

    /// Delete all records matching the query.
    pub async fn delete<M>(&mut self, query: stmt::Query<M>) -> Result<()> {
        self.exec(query.delete().into()).await?;
        Ok(())
    }

    /// Execute a statement, returning a raw value stream.
    pub async fn exec<M>(&mut self, statement: Statement<M>) -> Result<ValueStream> {
        let untyped = statement.untyped;
        self.exec_untyped(untyped).await
    }

    /// Execute a statement, expecting exactly one record.
    #[doc(hidden)]
    pub async fn exec_one<M>(&mut self, statement: Statement<M>) -> Result<stmt::Value> {
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
    pub async fn exec_insert_one<M: Model>(&mut self, mut stmt: stmt::Insert<M>) -> Result<M> {
        // TODO: HAX
        stmt.untyped.source.single = false;

        let mut records = self.exec(stmt.into()).await?;

        match records.next().await {
            Some(Ok(value)) => M::load(value),
            Some(Err(err)) => Err(err),
            None => Err(toasty_core::Error::record_not_found(
                "insert returned no results",
            )),
        }
    }
}
