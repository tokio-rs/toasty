use super::*;

pub struct Delete<'a, M: ?Sized> {
    pub(crate) untyped: stmt::Delete<'a>,
    _p: PhantomData<M>,
}

impl<'a, M: Model> Delete<'a, M> {
    pub const fn from_untyped(untyped: stmt::Delete<'a>) -> Delete<'a, M> {
        Delete {
            untyped,
            _p: PhantomData,
        }
    }
}

impl<'a, M> From<Delete<'a, M>> for Statement<'a, M> {
    fn from(value: Delete<'a, M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}
