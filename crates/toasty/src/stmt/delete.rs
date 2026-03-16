use super::{IntoStatement, Statement};
use crate::{Executor, ExecutorExt, Model, Result};
use std::marker::PhantomData;
use toasty_core::stmt;

pub struct Delete<M: ?Sized> {
    pub(crate) untyped: stmt::Delete,
    _p: PhantomData<M>,
}

impl<M> Delete<M> {
    pub const fn from_untyped(untyped: stmt::Delete) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

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
