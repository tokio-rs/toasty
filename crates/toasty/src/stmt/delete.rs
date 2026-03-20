use super::{IntoStatement, Statement};
use crate::{schema::Model, Executor, ExecutorExt, Result};
use std::marker::PhantomData;
use toasty_core::stmt;

/// A typed delete statement.
///
/// `Delete<M>` removes records of model `M` that match the selection built by
/// the originating [`Query`]. Obtain one by calling [`Query::delete`].
///
/// # Execution
///
/// Call [`exec`](Delete::exec) to run the delete, or convert it into a
/// [`Statement`] with [`IntoStatement`] for batch execution.
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
/// use toasty::stmt::Query;
///
/// Query::<User>::filter(User::fields().id().eq(1))
///     .delete()
///     .exec(&mut db)
///     .await
///     .unwrap();
/// # });
/// ```
pub struct Delete<M: ?Sized> {
    pub(crate) untyped: stmt::Delete,
    _p: PhantomData<M>,
}

impl<M> Delete<M> {
    /// Wrap a raw untyped [`stmt::Delete`](toasty_core::stmt::Delete).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{Delete, Query};
    ///
    /// // Build a delete from a query, then extract the raw form
    /// let delete = Query::<User>::all().delete();
    /// // The typed Delete wraps an untyped core delete
    /// let _: Delete<User> = delete;
    /// ```
    pub const fn from_untyped(untyped: stmt::Delete) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    /// Execute this delete statement against the given executor.
    ///
    /// Returns `Ok(())` on success. Any matching records are removed from the
    /// database.
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
    /// User::filter(User::fields().id().eq(1))
    ///     .delete()
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<()> {
        let stmt: Statement<M> = self.into();
        executor.exec(stmt).await?;
        Ok(())
    }
}

impl<M: Model> IntoStatement for Delete<M> {
    type Returning = ();

    fn into_statement(self) -> Statement<()> {
        Statement {
            untyped: self.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Delete<M>> for Statement<M> {
    fn from(value: Delete<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}
