use super::{IntoStatement, Statement};
use crate::{Executor, Result, schema::Load};
use std::marker::PhantomData;
use toasty_core::stmt;

/// A typed delete statement.
///
/// `Delete<T>` removes records that match the selection built by the
/// originating [`Query`]. Obtain one by calling [`Query::delete`].
///
/// The type parameter `T` is the **returning type**, not the model being
/// deleted. Currently `Query::delete` always produces `Delete<()>` because
/// deletes do not return the removed records.
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
/// # let mut db = toasty::Db::builder().models(toasty::models!(User)).build(driver).await.unwrap();
/// # db.push_schema().await.unwrap();
/// use toasty::stmt::{List, Query};
///
/// Query::<List<User>>::filter(User::fields().id().eq(1))
///     .delete()
///     .exec(&mut db)
///     .await
///     .unwrap();
/// # });
/// ```
pub struct Delete<T> {
    pub(crate) untyped: stmt::Delete,
    _p: PhantomData<T>,
}

impl<T> Delete<T> {
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
    /// use toasty::stmt::{Delete, List, Query};
    ///
    /// // Build a delete from a query
    /// let delete: Delete<()> = Query::<List<User>>::all().delete();
    /// ```
    pub const fn from_untyped(untyped: stmt::Delete) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }
}

impl<T> Delete<T> {
    /// Attach a condition to this delete. The condition is evaluated after the
    /// filter; if it fails, the operation returns an error (unlike a filter
    /// failure, which silently produces count 0).
    pub fn set_condition(mut self, condition: toasty_core::stmt::Condition) -> Self {
        self.untyped.condition = condition;
        self
    }
}

impl<T: Load> Delete<T> {
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
    /// # let mut db = toasty::Db::builder().models(toasty::models!(User)).build(driver).await.unwrap();
    /// # db.push_schema().await.unwrap();
    /// User::filter(User::fields().id().eq(1))
    ///     .delete()
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T::Output> {
        executor.exec(self.into()).await
    }
}

impl<T> IntoStatement for Delete<T> {
    type Returning = T;

    fn into_statement(self) -> Statement<T> {
        Statement {
            untyped: self.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<T> From<Delete<T>> for Statement<T> {
    fn from(value: Delete<T>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}
