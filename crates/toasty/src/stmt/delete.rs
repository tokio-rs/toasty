use super::*;

pub struct Delete<M: ?Sized> {
    pub(crate) untyped: stmt::Delete,
    _p: PhantomData<M>,
}

impl<M: Model> Delete<M> {
    pub const fn from_untyped(untyped: stmt::Delete) -> Delete<M> {
        Delete {
            untyped,
            _p: PhantomData,
        }
    }
}

impl<M> From<Delete<M>> for Statement<M> {
    fn from(value: Delete<M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}
