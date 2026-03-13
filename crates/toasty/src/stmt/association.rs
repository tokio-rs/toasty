use super::{IntoExpr, IntoStatement, List, Path, Select, Statement};
use crate::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

pub struct Association<T> {
    pub(crate) untyped: stmt::Association,
    _p: PhantomData<T>,
}

impl<M: Model> Association<List<M>> {
    /// A basic has_many association
    pub fn many<T: Model>(source: Select<T>, path: Path<List<M>>) -> Self {
        assert_eq!(path.untyped.root.expect_model(), T::id());

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
        assert_eq!(path.untyped.root.expect_model(), T::id());

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }

    pub fn insert(self, expr: impl IntoExpr<List<M>>) -> Statement<M> {
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

    /// Convert this association into a select query.
    pub fn into_select(self) -> Select<M> {
        Select::from_untyped(
            stmt::Query::builder(stmt::SourceModel {
                model: M::id(),
                via: Some(self.untyped),
            })
            .build(),
        )
    }
}

impl<T: Model> IntoStatement for Association<List<T>> {
    type Output = List<T>;

    fn into_statement(self) -> Statement<List<T>> {
        self.into_select().into_statement()
    }
}

impl<M: Model> Association<M> {
    pub fn one<T: Model>(source: Select<T>, path: Path<M>) -> Self {
        assert_eq!(path.untyped.root.expect_model(), T::id());

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }

    /// Convert this association into a select query.
    pub fn into_select(self) -> Select<M> {
        Select::from_untyped(
            stmt::Query::builder(stmt::SourceModel {
                model: M::id(),
                via: Some(self.untyped),
            })
            .build(),
        )
    }
}

impl<M> fmt::Debug for Association<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
