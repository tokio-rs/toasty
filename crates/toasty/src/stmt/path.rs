use super::{Expr, IntoExpr, IntoSelect};
use crate::Register;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt::{self, Direction, OrderByExpr};

pub struct Path<T: ?Sized> {
    pub(super) untyped: stmt::Path,
    _p: PhantomData<T>,
}

impl<T: ?Sized> Path<T> {
    pub const fn new(raw: stmt::Path) -> Self {
        Self {
            untyped: raw,
            _p: PhantomData,
        }
    }

    pub fn root() -> Self
    where
        T: Register,
    {
        Self {
            untyped: stmt::Path {
                root: T::id(),
                projection: stmt::Projection::identity(),
            },
            _p: PhantomData,
        }
    }

    pub fn from_field_index<M: Register>(index: usize) -> Self {
        Self {
            untyped: stmt::Path::from_index(M::id(), index),
            _p: PhantomData,
        }
    }

    pub fn chain<U: ?Sized>(mut self, other: impl Into<Path<U>>) -> Path<U> {
        let other = other.into();
        self.untyped.chain(&other.untyped);

        Path {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }

    pub fn eq(self, rhs: impl IntoExpr<T>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::eq(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn ne(self, rhs: impl IntoExpr<T>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ne(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn gt(self, rhs: impl IntoExpr<T>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::gt(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn ge(self, rhs: impl IntoExpr<T>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ge(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn lt(self, rhs: impl IntoExpr<T>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::lt(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn le(self, rhs: impl IntoExpr<T>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::le(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn in_set(self, rhs: impl IntoExpr<[T]>) -> Expr<bool>
    where
        T: Sized,
    {
        Expr {
            untyped: stmt::Expr::in_list(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn in_query(self, rhs: impl IntoSelect<Model = T>) -> Expr<bool>
    where
        T: Sized,
    {
        Expr {
            untyped: stmt::Expr::in_subquery(self.untyped.into_stmt(), rhs.into_select().untyped),
            _p: PhantomData,
        }
    }

    pub fn asc(self) -> OrderByExpr {
        OrderByExpr {
            expr: self.untyped.into_stmt(),
            order: Some(Direction::Asc),
        }
    }

    pub fn desc(self) -> OrderByExpr {
        OrderByExpr {
            expr: self.untyped.into_stmt(),
            order: Some(Direction::Desc),
        }
    }
}

impl<M> Path<M> {}

impl<T> Clone for Path<T> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<T> IntoExpr<T> for Path<T> {
    fn into_expr(self) -> Expr<T> {
        Expr {
            untyped: self.untyped.into_stmt(),
            _p: PhantomData,
        }
    }

    fn by_ref(&self) -> Expr<T> {
        Self::into_expr(self.clone())
    }
}

impl<T: ?Sized> From<Path<T>> for stmt::Path {
    fn from(value: Path<T>) -> Self {
        value.untyped
    }
}

impl<T: ?Sized> fmt::Debug for Path<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.untyped)
    }
}
