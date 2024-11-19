use super::*;

use std::{fmt, marker::PhantomData};

pub struct Unlink<M: ?Sized> {
    pub(crate) untyped: stmt::Unlink,
    _p: PhantomData<M>,
}

impl<M: Model + ?Sized> Unlink<M> {
    pub fn new<T: Model>(
        source: impl IntoSelect<Model = M>,
        path: impl Into<Path<[T]>>,
        expr: impl IntoSelect<Model = T>,
    ) -> Unlink<M> {
        let path = path.into();
        let source = source.into_select().untyped;
        let target = expr.into_select().untyped;

        Unlink {
            untyped: stmt::Unlink {
                source: source,
                field: path.to_field_id::<M>(),
                target: target,
            },
            _p: PhantomData,
        }
    }
}

impl<M> From<Unlink<M>> for Statement<M> {
    fn from(value: Unlink<M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Unlink<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
