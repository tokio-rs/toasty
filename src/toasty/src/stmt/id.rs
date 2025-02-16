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
}

impl<M> std::fmt::Display for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<M: Model> IntoExpr<Id<M>> for Id<M> {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(self.inner.into())
    }
}

impl<M: Model> IntoExpr<Id<M>> for &Id<M> {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(Value::from(&self.inner))
    }
}

impl<M: Model> IntoExpr<Id<M>> for String {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::ID, self).into())
    }
}

impl<M: Model> IntoExpr<Id<M>> for &String {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::ID, self.clone()).into())
    }
}

impl<M: Model> IntoExpr<Id<M>> for &str {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::ID, self.into()).into())
    }
}

impl<M: Model> From<Id<M>> for stmt::Expr {
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

impl<M> From<Id<M>> for Value {
    fn from(value: Id<M>) -> Self {
        Value::from(value.inner)
    }
}

impl<M> From<&Id<M>> for Value {
    fn from(src: &Id<M>) -> Value {
        Value::from(&src.inner)
    }
}

impl<M> From<&Id<M>> for Id<M> {
    fn from(src: &Id<M>) -> Id<M> {
        src.clone()
    }
}

impl<M> From<&Id<M>> for stmt::Expr {
    fn from(value: &Id<M>) -> Self {
        stmt::Expr::from(&value.inner)
    }
}

impl<M> Eq for Id<M> {}

impl<M> Hash for Id<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
