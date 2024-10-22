use super::*;

use std::{fmt, marker::PhantomData};

// TODO: rename to Query?
pub struct Select<'a, M> {
    /// How to filter the data source
    pub(crate) untyped: stmt::Query<'a>,

    _p: PhantomData<M>,
}

impl<'a, M: Model> Select<'a, M> {
    pub fn unit() -> Select<'a, M> {
        Select {
            untyped: stmt::Query::unit(),
            _p: PhantomData,
        }
    }

    pub(crate) const fn from_untyped(untyped: stmt::Query<'a>) -> Select<'a, M> {
        Select {
            untyped,
            _p: PhantomData,
        }
    }

    pub fn from_expr(expr: Expr<'a, bool>) -> Select<'a, M> {
        Select::from_untyped(stmt::Query::filter(M::ID, expr.untyped))
    }

    // TODO: why are these by value?
    pub fn and(mut self, filter: Expr<'a, bool>) -> Select<'a, M> {
        self.untyped.and(filter.untyped);
        self
    }

    pub fn union(mut self, other: Select<'a, M>) -> Select<'a, M> {
        self.untyped.union(other.untyped);
        self
    }

    pub fn include(&mut self, path: impl Into<stmt::Path>) -> &mut Self {
        self.untyped.include(path.into());
        self
    }

    // TODO: not quite right
    pub fn delete(self) -> Statement<'a, M> {
        Delete::from_untyped(self.untyped.delete()).into()
    }
}

impl<M: Model> Select<'static, M> {
    pub fn all() -> Select<'static, M> {
        let filter = stmt::Expr::Value(Value::from_bool(true));
        Select::from_untyped(stmt::Query::filter(M::ID, filter))
    }
}

impl<'a, M: Model> IntoSelect<'a> for &'a Select<'_, M> {
    type Model = M;

    fn into_select(self) -> Select<'a, M> {
        self.clone()
    }
}

impl<M> Clone for Select<'_, M> {
    fn clone(&self) -> Self {
        Select {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<'a, M> fmt::Debug for Select<'a, M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
