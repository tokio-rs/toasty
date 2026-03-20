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
/// ```ignore
/// User::find_by_id(&id).delete().exec(&mut db).await?;
/// ```
pub struct Delete<M: ?Sized> {
    pub(crate) untyped: stmt::Delete,
    _p: PhantomData<M>,
}

impl<M> Delete<M> {
    /// Wrap a raw untyped [`stmt::Delete`](toasty_core::stmt::Delete).
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
