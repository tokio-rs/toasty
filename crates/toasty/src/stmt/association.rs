use super::*;

use std::{fmt, marker::PhantomData};

pub struct Association<T: ?Sized> {
    pub(crate) untyped: stmt::Association,
    _p: PhantomData<T>,
}

impl<M: Model> Association<[M]> {
    /// A basic has_many association
    pub fn many<T: Model>(source: Select<T>, path: Path<[M]>) -> Self {
        assert_eq!(path.untyped.root, T::ID);

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }

    /// A has_one or belongs_to association via a query, which implies there
    /// could be more than one result.
    pub fn many_via_one<T: Model>(source: Select<T>, path: Path<M>) -> Self {
        assert_eq!(path.untyped.root, T::ID);

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }

    pub fn insert(self, expr: impl IntoExpr<[M]>) -> Statement<M> {
        let [index] = self.untyped.path.projection.as_slice() else {
            todo!()
        };

        let mut stmt = self.untyped.source.update();
        stmt.assignments.insert(*index, expr.into_expr().untyped);

        Statement {
            untyped: stmt.into(),
            _p: PhantomData,
        }
    }

    pub fn remove(self, expr: impl IntoExpr<M>) -> Statement<M> {
        let [index] = self.untyped.path.projection.as_slice() else {
            todo!()
        };
        let mut stmt = self.untyped.source.update();
        stmt.assignments.remove(*index, expr.into_expr().untyped);

        Statement {
            untyped: stmt.into(),
            _p: PhantomData,
        }
    }
}

impl<M: Model> Association<M> {
    pub fn one<T: Model>(source: Select<T>, path: Path<M>) -> Self {
        assert_eq!(path.untyped.root, T::ID);

        Self {
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

impl<T: Model> IntoSelect for Association<[T]> {
    type Model = T;

    fn into_select(self) -> Select<T> {
        Select::from_untyped(
            stmt::Query::builder(stmt::SourceModel {
                model: T::ID,
                via: Some(self.untyped),
                include: vec![],
            })
            .build(),
        )
    }
}

impl<T: Model> IntoSelect for Association<T> {
    type Model = T;

    fn into_select(self) -> Select<T> {
        Select::from_untyped(
            stmt::Query::builder(stmt::SourceModel {
                model: T::ID,
                via: Some(self.untyped),
                include: vec![],
            })
            .build(),
        )
    }
}
