use super::*;

use std::{fmt, marker::PhantomData};

pub struct Link<'stmt, M: ?Sized> {
    pub(crate) untyped: stmt::Link<'stmt>,
    _p: PhantomData<M>,
}

impl<'stmt, M: Model + ?Sized> Link<'stmt, M> {
    pub fn new<T: Model>(
        source: impl IntoSelect<'stmt, Model = M>,
        path: impl Into<Path<[T]>>,
        expr: impl IntoSelect<'stmt, Model = T>,
    ) -> Link<'stmt, M> {
        let path = path.into();
        let source = source.into_select().untyped;
        let target = expr.into_select().untyped;

        Link {
            untyped: stmt::Link {
                source: source,
                field: path.to_field_id::<M>(),
                target: target,
            },
            _p: PhantomData,
        }
    }
}

impl<'a, M> From<Link<'a, M>> for Statement<'a, M> {
    fn from(value: Link<'a, M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<'a, M> fmt::Debug for Link<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
