use super::*;

use std::{fmt, marker::PhantomData};

pub struct Association<M: ?Sized> {
    pub(crate) untyped: stmt::Association,
    _p: PhantomData<M>,
}

impl<M: ?Sized> Association<M> {
    pub fn new<T: Model>(source: Select<T>, path: Path<M>) -> Association<M> {
        assert_eq!(path.untyped.root, T::ID);

        Association {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }
}

impl<M: ?Sized> fmt::Debug for Association<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}

impl<T: Model> IntoSelect for Association<T> {
    type Model = T;

    fn into_select(self) -> Select<T> {
        Select::from_untyped(stmt::Query {
            body: Box::new(stmt::ExprSet::Select(stmt::Select {
                source: stmt::Source::Model(stmt::SourceModel {
                    model: T::ID,
                    via: Some(self.untyped),
                    include: vec![],
                }),
                filter: true.into(),
                returning: stmt::Returning::Star,
            })),
        })
    }
}

impl<T: Model + ?Sized> IntoSelect for Association<[T]> {
    type Model = T;

    fn into_select(self) -> Select<T> {
        Select::from_untyped(stmt::Query {
            body: Box::new(stmt::ExprSet::Select(stmt::Select {
                source: stmt::Source::Model(stmt::SourceModel {
                    model: T::ID,
                    via: Some(self.untyped),
                    include: vec![],
                }),
                filter: true.into(),
                returning: stmt::Returning::Star,
            })),
        })
    }
}
