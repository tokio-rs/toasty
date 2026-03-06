use crate::{model::Load, Error};
use toasty_core::stmt;

pub struct Cursor<M> {
    values: stmt::ValueStream,
    _p: std::marker::PhantomData<M>,
}

pub trait FromCursor<A>: Extend<A> + Default {}

impl<A, T: Extend<A> + Default> FromCursor<A> for T {}

impl<M: Load> Cursor<M> {
    pub(crate) fn new(values: stmt::ValueStream) -> Self {
        Self {
            values,
            _p: std::marker::PhantomData,
        }
    }

    pub async fn next(&mut self) -> Option<Result<M, Error>> {
        Some(match self.values.next().await? {
            Ok(value) => M::load(value),
            Err(e) => Err(e),
        })
    }

    /// Collect all values
    pub async fn collect<B>(mut self) -> Result<B, Error>
    where
        B: FromCursor<M>,
    {
        let mut ret = B::default();

        while let Some(res) = self.next().await {
            ret.extend(Some(res?));
        }

        Ok(ret)
    }
}
