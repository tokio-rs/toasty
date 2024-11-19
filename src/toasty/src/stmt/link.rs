use super::*;

use std::{fmt, marker::PhantomData};

pub struct Link<M: ?Sized> {
    pub(crate) untyped: stmt::Link,
    _p: PhantomData<M>,
}

impl<M: Model + ?Sized> Link<M> {
    pub fn new<T: Model>(
        source: impl IntoSelect<Model = M>,
        path: impl Into<Path<[T]>>,
        expr: impl IntoSelect<Model = T>,
    ) -> Link<M> {
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

impl<M> From<Link<M>> for Statement<M> {
    fn from(value: Link<M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Link<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
