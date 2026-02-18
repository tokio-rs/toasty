use super::{Insert, IntoExpr};
use std::marker::PhantomData;
use std::ops::Not;
use toasty_core::stmt;

#[derive(Debug)]
pub struct Expr<T: ?Sized> {
    /// The un-typed expression
    pub(crate) untyped: stmt::Expr,

    /// `T` is the type of the expression
    pub(crate) _p: PhantomData<T>,
}

impl<T: ?Sized> Expr<T> {
    /// Create an expression from the given value.
    pub(crate) fn from_value(value: stmt::Value) -> Self {
        Self {
            untyped: stmt::Expr::Value(value),
            _p: PhantomData,
        }
    }

    pub fn from_untyped(untyped: impl Into<stmt::Expr>) -> Self {
        Self {
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
    pub fn list<I>(items: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoExpr<T>,
    {
        Self::from_untyped(stmt::Expr::list(
            items.into_iter().map(|item| item.into_expr().untyped),
        ))
    }
}

impl Expr<bool> {
    pub fn and(self, rhs: impl IntoExpr<bool>) -> Self {
        Self::from_untyped(stmt::Expr::and(self.untyped, rhs.into_expr().untyped))
    }

    pub fn and_all<E>(exprs: impl IntoIterator<Item = E>) -> Self
    where
        E: IntoExpr<bool>,
    {
        exprs
            .into_iter()
            .map(|expr| expr.into_expr().untyped)
            .reduce(stmt::Expr::and)
            .map(Self::from_untyped)
            .unwrap_or_else(|| Self::from_untyped(true))
    }

    pub fn or(self, rhs: impl IntoExpr<bool>) -> Self {
        Self::from_untyped(stmt::Expr::or(self.untyped, rhs.into_expr().untyped))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        !self
    }

    pub fn in_list<L, R, T>(lhs: L, rhs: R) -> Self
    where
        L: IntoExpr<T>,
        R: IntoExpr<[T]>,
    {
        Self::from_untyped(stmt::Expr::in_list(
            lhs.into_expr().untyped,
            rhs.into_expr().untyped,
        ))
    }
}

impl Not for Expr<bool> {
    type Output = Self;

    fn not(self) -> Self {
        Self::from_untyped(stmt::Expr::not(self.untyped))
    }
}

impl<T: ?Sized> Clone for Expr<T> {
    fn clone(&self) -> Self {
        Self {
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
        Self::from_untyped(stmt::Expr::Stmt(value.untyped.into()))
    }
}

impl<T> From<Insert<T>> for Expr<Option<T>> {
    fn from(value: Insert<T>) -> Self {
        Self::from_untyped(stmt::Expr::Stmt(value.untyped.into()))
    }
}
