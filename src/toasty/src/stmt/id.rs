use crate::{
    stmt::{Expr, IntoExpr},
    Model,
};
use toasty_core::stmt::{self, Value};

use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

pub struct Id<M> {
    pub(crate) inner: stmt::Id,
    _p: PhantomData<M>,
}

impl<M> Id<M> {
    pub fn from_untyped(id: stmt::Id) -> Id<M> {
        Id {
            inner: id,
            _p: PhantomData,
        }
    }

    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

impl<'a, M: Model> IntoExpr<'a, Id<M>> for Id<M> {
    fn into_expr(self) -> Expr<'a, Id<M>> {
        Expr::from_value(self.inner.into())
    }
}

impl<'a, M: Model> IntoExpr<'a, Id<M>> for &'a Id<M> {
    fn into_expr(self) -> Expr<'a, Id<M>> {
        Expr::from_value(Value::from(&self.inner))
    }
}

impl<'a, M: Model> IntoExpr<'a, Id<M>> for String {
    fn into_expr(self) -> Expr<'a, Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::ID, self).into())
    }
}

impl<'a, M: Model> IntoExpr<'a, Id<M>> for &'a String {
    fn into_expr(self) -> Expr<'a, Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::ID, self.clone()).into())
    }
}

impl<'a, M: Model> IntoExpr<'a, Id<M>> for &'a str {
    fn into_expr(self) -> Expr<'a, Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::ID, self.into()).into())
    }
}

impl<'a, M: Model> From<Id<M>> for stmt::Expr<'a> {
    fn from(value: Id<M>) -> Self {
        stmt::Expr::Value(value.inner.into())
    }
}

impl<M> Clone for Id<M> {
    fn clone(&self) -> Id<M> {
        Id {
            inner: self.inner.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> PartialEq for Id<M> {
    fn eq(&self, rhs: &Id<M>) -> bool {
        self.inner.eq(&rhs.inner)
    }
}

impl<M> fmt::Debug for Id<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(fmt)
    }
}

impl<'a, M> From<Id<M>> for Value<'a> {
    fn from(value: Id<M>) -> Self {
        Value::from(value.inner)
    }
}

impl<'a, M> From<&'a Id<M>> for Value<'a> {
    fn from(src: &'a Id<M>) -> Value<'a> {
        Value::from(&src.inner)
    }
}

impl<'a, M> From<&'a Id<M>> for Id<M> {
    fn from(src: &'a Id<M>) -> Id<M> {
        src.clone()
    }
}

impl<'a, M> From<&'a Id<M>> for stmt::Expr<'a> {
    fn from(value: &'a Id<M>) -> Self {
        stmt::Expr::from(&value.inner)
    }
}

impl<'a, M> Eq for Id<M> {}

impl<'a, M> Hash for Id<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
