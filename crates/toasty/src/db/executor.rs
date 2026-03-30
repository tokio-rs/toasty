use crate::{Result, Statement, db::Transaction, schema::Load};

use crate::engine::exec::ExecResponse;
use async_trait::async_trait;
use std::sync::Arc;
use toasty_core::{Schema, stmt::Value};

/// Anything that can execute queries — [`Db`](crate::Db) or
/// [`Transaction`](crate::db::Transaction).
///
/// This trait is dyn-compatible. The generic [`exec`](dyn Executor::exec)
/// method lives on `dyn Executor` and accepts any typed
/// [`Statement<T>`](crate::Statement).
#[async_trait]
pub trait Executor: Send + Sync {
    /// Starts a (potentially nested) transaction.
    async fn transaction(&mut self) -> Result<Transaction<'_>>;

    /// Execute an untyped statement, returning a raw value stream.
    #[doc(hidden)]
    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<Value>;

    /// Execute an untyped statement, returning the full response with pagination metadata.
    #[doc(hidden)]
    async fn exec_paginated(&mut self, stmt: toasty_core::stmt::Statement) -> Result<ExecResponse>;

    /// Returns the schema associated with this executor.
    #[doc(hidden)]
    fn schema(&mut self) -> &Arc<Schema>;
}

impl dyn Executor + '_ {
    /// Execute a typed [`Statement`] and deserialize the result.
    ///
    /// This is the primary entry point for running queries, inserts, updates,
    /// and deletes. The return type is determined by the statement's type
    /// parameter `T`:
    ///
    /// - `Statement<List<M>>` returns `Vec<M>`.
    /// - `Statement<M>` returns `M`.
    /// - `Statement<Option<M>>` returns `Option<M>`.
    /// - `Statement<()>` returns `()`.
    ///
    /// Most users call `exec` on the query/update/delete builders directly
    /// (e.g., [`Query::exec`](crate::stmt::Query::exec)) rather than calling
    /// this method, but it is available for working with [`Statement`] values
    /// directly.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// # let mut db = toasty::Db::builder().register::<User>().build(driver).await.unwrap();
    /// # db.push_schema().await.unwrap();
    /// use toasty::stmt::{IntoStatement, List, Query};
    /// use toasty::Executor;
    ///
    /// let stmt = Query::<List<User>>::all().into_statement();
    /// let executor: &mut dyn Executor = &mut db;
    /// let users: Vec<User> = executor.exec(stmt).await.unwrap();
    /// # });
    /// ```
    pub async fn exec<T: Load>(&mut self, stmt: Statement<T>) -> Result<T::Output> {
        let res = self.exec_untyped(stmt.untyped).await?;
        T::load(res)
    }
}
