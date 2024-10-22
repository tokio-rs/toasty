use super::*;

use std::marker::PhantomData;

#[derive(Debug)]
pub struct Expr<'a, T: ?Sized> {
    /// The un-typed expression
    pub(crate) untyped: stmt::Expr<'a>,

    /// `M` and `T` are just used to verify usage.
    pub(crate) _p: PhantomData<T>,
}

impl<'a, T: ?Sized> Expr<'a, T> {
    /// Create an expression from the given value.
    pub(crate) fn from_value(value: stmt::Value<'a>) -> Expr<'a, T> {
        Expr {
            untyped: stmt::Expr::Value(value),
            _p: PhantomData,
        }
    }

    pub fn from_untyped(untyped: impl Into<stmt::Expr<'a>>) -> Expr<'a, T> {
        Expr {
            untyped: untyped.into(),
            _p: PhantomData,
        }
    }

    pub fn cast<U: ?Sized>(self) -> Expr<'a, U> {
        Expr {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }
}

impl<'stmt> Expr<'stmt, bool> {
    pub fn and(self, rhs: impl IntoExpr<'stmt, bool>) -> Expr<'stmt, bool> {
        Expr::from_untyped(stmt::Expr::and(self.untyped, rhs.into_expr().untyped))
    }
}

impl<'stmt, T: ?Sized> From<Expr<'stmt, T>> for stmt::Expr<'stmt> {
    fn from(value: Expr<'stmt, T>) -> Self {
        value.untyped
    }
}

impl<'stmt, T: ?Sized> From<Insert<'stmt, T>> for Expr<'stmt, T> {
    fn from(value: Insert<'stmt, T>) -> Self {
        Expr::from_untyped(stmt::Expr::Stmt(value.untyped.into()))
    }
}
