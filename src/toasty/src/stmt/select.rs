use super::*;

use std::{fmt, marker::PhantomData};

// TODO: rename to Query?
pub struct Select<M> {
    /// How to filter the data source
    pub(crate) untyped: stmt::Query,

    _p: PhantomData<M>,
}

impl<M: Model> Select<M> {
    pub fn unit() -> Select<M> {
        Select {
            untyped: stmt::Query::unit(),
            _p: PhantomData,
        }
    }

    pub(crate) const fn from_untyped(untyped: stmt::Query) -> Select<M> {
        Select {
            untyped,
            _p: PhantomData,
        }
    }

    /// Convert a model expression to a query
    pub fn from_expr(expr: Expr<M>) -> Select<M> {
        match expr.untyped {
            stmt::Expr::Stmt(expr) => match *expr.stmt {
                stmt::Statement::Query(stmt) => Select::from_untyped(stmt),
                stmt => todo!("stmt={stmt:#?}"),
            },
            expr => Select::from_untyped(stmt::Query::values(expr)),
        }
    }

    pub fn filter(expr: Expr<bool>) -> Select<M> {
        Select::from_untyped(stmt::Query::filter(M::ID, expr.untyped))
    }

    // TODO: why are these by value?
    pub fn and(mut self, filter: Expr<bool>) -> Select<M> {
        self.untyped.and(filter.untyped);
        self
    }

    pub fn union(mut self, other: Select<M>) -> Select<M> {
        self.untyped.union(other.untyped);
        self
    }

    pub fn include(&mut self, path: impl Into<stmt::Path>) -> &mut Self {
        self.untyped.include(path.into());
        self
    }

    // TODO: not quite right
    pub fn delete(self) -> Statement<M> {
        Delete::from_untyped(self.untyped.delete()).into()
    }
}

impl<M: Model> Select<M> {
    pub fn all() -> Select<M> {
        let filter = stmt::Expr::Value(Value::from_bool(true));
        Select::from_untyped(stmt::Query::filter(M::ID, filter))
    }
}

impl<M: Model> IntoSelect for &Select<M> {
    type Model = M;

    fn into_select(self) -> Select<M> {
        self.clone()
    }
}

impl<M> Clone for Select<M> {
    fn clone(&self) -> Self {
        Select {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Select<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
