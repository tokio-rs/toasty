use crate::{
    stmt::{Expr, IntoExpr},
    Model,
};
use toasty_core::stmt::{self, Value};

use std::{
    convert::TryFrom,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

pub struct Id<M> {
    pub(crate) inner: stmt::Id,
    _p: PhantomData<M>,
}

impl<M> Id<M> {
    pub fn from_untyped(id: stmt::Id) -> Self {
        Self {
            inner: id,
            _p: PhantomData,
        }
    }
}

impl<M: Model> Id<M> {
    /// Create an `Id` from the model's string representation.
    pub fn from_string(id: impl Into<String>) -> Self {
        Self::from_untyped(stmt::Id::from_string(M::id(), id.into()))
    }

    /// Create an `Id` from an unsigned integer.
    pub fn from_u64(id: u64) -> Self {
        Self::from_untyped(stmt::Id::from_int(M::id(), id))
    }

    /// Create an `Id` from a signed integer. Negative inputs panic.
    pub fn from_i64(id: i64) -> Self {
        let value = u64::try_from(id).expect("Id values must be non-negative");
        Self::from_u64(value)
    }
}

impl<M> std::fmt::Display for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<M: Model> IntoExpr<Self> for Id<M> {
    fn into_expr(self) -> Expr<Self> {
        Expr::from_value(self.inner.into())
    }

    fn by_ref(&self) -> Expr<Self> {
        Expr::from_value((&self.inner).into())
    }
}

impl<M: Model> IntoExpr<Id<M>> for String {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::id(), self).into())
    }

    fn by_ref(&self) -> Expr<Id<M>> {
        Self::into_expr(self.clone())
    }
}

impl<M: Model> IntoExpr<Id<M>> for &str {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::id(), self.into()).into())
    }

    fn by_ref(&self) -> Expr<Id<M>> {
        Self::into_expr(*self)
    }
}

impl<M: Model> IntoExpr<Id<M>> for &String {
    fn into_expr(self) -> Expr<Id<M>> {
        Expr::from_value(stmt::Id::from_string(M::id(), self.into()).into())
    }

    fn by_ref(&self) -> Expr<Id<M>> {
        Self::into_expr(*self)
    }
}

fn expr_from_int<M: Model>(value: u64) -> Expr<Id<M>> {
    Expr::from_value(stmt::Id::from_int(M::id(), value).into())
}

macro_rules! impl_into_expr_id_unsigned {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl<M: Model> IntoExpr<Id<M>> for $ty {
                fn into_expr(self) -> Expr<Id<M>> {
                    expr_from_int::<M>(u64::from(self))
                }

                fn by_ref(&self) -> Expr<Id<M>> {
                    expr_from_int::<M>(u64::from(*self))
                }
            }
        )+
    };
}

macro_rules! impl_into_expr_id_signed {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl<M: Model> IntoExpr<Id<M>> for $ty {
                fn into_expr(self) -> Expr<Id<M>> {
                    let value = u64::try_from(self).expect("Id values must be non-negative");
                    expr_from_int::<M>(value)
                }

                fn by_ref(&self) -> Expr<Id<M>> {
                    let value = u64::try_from(*self).expect("Id values must be non-negative");
                    expr_from_int::<M>(value)
                }
            }
        )+
    };
}

impl_into_expr_id_unsigned!(u8, u16, u32, u64);
impl_into_expr_id_signed!(i8, i16, i32, i64, isize);

impl<M: Model> IntoExpr<Id<M>> for usize {
    fn into_expr(self) -> Expr<Id<M>> {
        let value = u64::try_from(self).expect("Id values must fit in u64");
        expr_from_int::<M>(value)
    }

    fn by_ref(&self) -> Expr<Id<M>> {
        let value = u64::try_from(*self).expect("Id values must fit in u64");
        expr_from_int::<M>(value)
    }
}

impl<M: Model> From<Id<M>> for stmt::Expr {
    fn from(value: Id<M>) -> Self {
        Self::Value(value.inner.into())
    }
}

impl<M> Clone for Id<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> PartialEq for Id<M> {
    fn eq(&self, rhs: &Self) -> bool {
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
        Self::from(value.inner)
    }
}

impl<M> From<&Id<M>> for Value {
    fn from(src: &Id<M>) -> Self {
        Self::from(&src.inner)
    }
}

impl<M> From<&Self> for Id<M> {
    fn from(src: &Self) -> Self {
        src.clone()
    }
}

impl<M> From<&Id<M>> for stmt::Expr {
    fn from(value: &Id<M>) -> Self {
        Self::from(&value.inner)
    }
}

impl<M> Eq for Id<M> {}

impl<M> Hash for Id<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

#[cfg(feature = "serde")]
impl<T> serde_core::Serialize for Id<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        match (self.inner.as_str(), self.inner.to_int()) {
            (Err(_), Ok(id)) => id.serialize(serializer),
            (Ok(id), Err(_)) => id.serialize(serializer),
            _ => unreachable!(),
        }
    }
}
