use super::*;

use std::marker::PhantomData;

#[derive(Debug)]
pub struct Expr<T: ?Sized> {
    /// The un-typed expression
    pub(crate) untyped: stmt::Expr,

    /// `M` and `T` are just used to verify usage.
    pub(crate) _p: PhantomData<T>,
}

impl<T: ?Sized> Expr<T> {
    /// Create an expression from the given value.
    pub(crate) fn from_value(value: stmt::Value) -> Expr<T> {
        Expr {
            untyped: stmt::Expr::Value(value),
            _p: PhantomData,
        }
    }

    pub fn from_untyped(untyped: impl Into<stmt::Expr>) -> Expr<T> {
        Expr {
            untyped: untyped.into(),
            _p: PhantomData,
        }
    }

    pub fn cast<U: ?Sized>(self) -> Expr<U> {
        Expr {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }
}

impl<T> Expr<[T]> {
    pub fn list<I>(items: impl IntoIterator<Item = I>) -> Expr<[T]>
    where
        I: IntoExpr<T>,
    {
        Expr::from_untyped(stmt::Expr::list(
            items.into_iter().map(|item| item.into_expr().untyped),
        ))
    }
}

impl Expr<bool> {
    pub fn and(self, rhs: impl IntoExpr<bool>) -> Expr<bool> {
        Expr::from_untyped(stmt::Expr::and(self.untyped, rhs.into_expr().untyped))
    }

    pub fn and_all<E>(exprs: impl IntoIterator<Item = E>) -> Expr<bool>
    where
        E: IntoExpr<bool>,
    {
        exprs
            .into_iter()
            .map(|expr| expr.into_expr().untyped)
            .reduce(stmt::Expr::and)
            .map(Expr::from_untyped)
            .unwrap_or_else(|| Expr::from_untyped(true))
    }

    pub fn in_list<L, R, T>(lhs: L, rhs: R) -> Expr<bool>
    where
        L: IntoExpr<T>,
        R: IntoExpr<[T]>,
    {
        Expr::from_untyped(stmt::Expr::in_list(
            lhs.into_expr().untyped,
            rhs.into_expr().untyped,
        ))
    }
}

impl<T: ?Sized> Clone for Expr<T> {
    fn clone(&self) -> Expr<T> {
        Expr {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<T: ?Sized> From<Expr<T>> for stmt::Expr {
    fn from(value: Expr<T>) -> Self {
        value.untyped
    }
}

impl<T: ?Sized> From<Insert<T>> for Expr<T> {
    fn from(value: Insert<T>) -> Self {
        Expr::from_untyped(stmt::Expr::Stmt(value.untyped.into()))
    }
}
