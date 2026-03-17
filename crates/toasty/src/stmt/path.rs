use super::{Expr, IntoExpr, IntoStatement, List};
use crate::Register;
use std::{fmt, marker::PhantomData};
use toasty_core::{
    schema::app::VariantId,
    stmt::{self, Direction, OrderByExpr},
};

pub struct Path<T, U> {
    pub(super) untyped: stmt::Path,
    _p: PhantomData<(T, U)>,
}

impl<T: Register> Path<T, T> {
    pub fn root() -> Self {
        Self {
            untyped: stmt::Path::model(T::id()),
            _p: PhantomData,
        }
    }
}

impl<T, U> Path<T, U> {
    pub const fn new(raw: stmt::Path) -> Self {
        Self {
            untyped: raw,
            _p: PhantomData,
        }
    }

    pub fn from_field_index(index: usize) -> Self
    where
        T: Register,
    {
        Self {
            untyped: stmt::Path::from_index(T::id(), index),
            _p: PhantomData,
        }
    }

    /// Converts this path into a variant-rooted path for use in `.matches()`
    /// closures on embedded enum fields.
    pub fn into_variant(self, variant_id: VariantId) -> Self {
        Self {
            untyped: stmt::Path::from_variant(self.untyped, variant_id),
            _p: PhantomData,
        }
    }

    pub fn chain<X, V>(mut self, other: impl Into<Path<X, V>>) -> Path<T, V> {
        let other = other.into();
        self.untyped.chain(&other.untyped);

        Path {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }

    pub fn eq(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::eq(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn ne(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ne(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn gt(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::gt(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn ge(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::ge(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn lt(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::lt(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn le(self, rhs: impl IntoExpr<U>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::le(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn in_list(self, rhs: impl IntoExpr<List<U>>) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::in_list(self.untyped.into_stmt(), rhs.into_expr().untyped),
            _p: PhantomData,
        }
    }

    pub fn in_query<Q>(self, rhs: Q) -> Expr<bool>
    where
        Q: IntoStatement<Returning = List<U>>,
    {
        let query = rhs.into_statement().into_untyped_query();
        Expr {
            untyped: stmt::Expr::in_subquery(self.untyped.into_stmt(), query),
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

impl<T, U> Path<T, List<U>> {
    /// Build an `IN subquery` expression that tests whether **any** associated
    /// record satisfies `filter`.
    ///
    /// The path must point to a `HasMany` (or similar collection) field on the
    /// parent model. The returned expression can be used as a filter on the
    /// parent query.
    pub fn any(self, filter: Expr<bool>) -> Expr<bool>
    where
        U: crate::Model,
    {
        // Build a query on the child model filtered by `filter`
        let child_query = super::Query::<U>::filter(filter);

        Expr {
            untyped: stmt::Expr::in_subquery(self.untyped.into_stmt(), child_query.untyped),
            _p: PhantomData,
        }
    }
}

impl<T, U> Path<T, Option<U>> {
    pub fn is_none(self) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::is_null(self.untyped.into_stmt()),
            _p: PhantomData,
        }
    }

    pub fn is_some(self) -> Expr<bool> {
        Expr {
            untyped: stmt::Expr::is_not_null(self.untyped.into_stmt()),
            _p: PhantomData,
        }
    }
}

impl<T, U> Clone for Path<T, U> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<T, U> IntoExpr<U> for Path<T, U> {
    fn into_expr(self) -> Expr<U> {
        Expr {
            untyped: self.untyped.into_stmt(),
            _p: PhantomData,
        }
    }

    fn by_ref(&self) -> Expr<U> {
        Self::into_expr(self.clone())
    }
}

impl<T, U> From<Path<T, U>> for stmt::Path {
    fn from(value: Path<T, U>) -> Self {
        value.untyped
    }
}

impl<T, U> fmt::Debug for Path<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.untyped)
    }
}
