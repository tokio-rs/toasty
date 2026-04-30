use std::marker::PhantomData;

use toasty_core::stmt;

use super::{Expr, Path};

/// A typed wrapper around an untyped [`stmt::Include`] that carries the
/// origin model and the relation target type.
///
/// You rarely build an `Include` directly — pass a relation path
/// (`User::fields().posts()`) to `.include(...)` and it is converted
/// automatically. Adding a per-relation predicate is what produces an
/// `Include` explicitly: `User::fields().posts().filter(...)`.
pub struct Include<Origin, T> {
    pub(crate) untyped: stmt::Include,
    _p: PhantomData<fn() -> (Origin, T)>,
}

impl<Origin, T> Include<Origin, T> {
    /// Build a typed `Include` from a typed path and predicate.
    ///
    /// The predicate is evaluated in the relation target's scope (the
    /// fields of `T` when `T` is a model, or the element type of `T`
    /// when `T` is a list).
    pub fn from_path_and_filter(path: Path<Origin, T>, filter: Expr<bool>) -> Self {
        Self {
            untyped: stmt::Include::with_filter(path.untyped, filter.untyped),
            _p: PhantomData,
        }
    }
}

impl<Origin, T> From<Path<Origin, T>> for Include<Origin, T> {
    fn from(path: Path<Origin, T>) -> Self {
        Self {
            untyped: stmt::Include::new(path.untyped),
            _p: PhantomData,
        }
    }
}

impl<Origin, T> From<Include<Origin, T>> for stmt::Include {
    fn from(value: Include<Origin, T>) -> Self {
        value.untyped
    }
}

impl<Origin, T> From<Path<Origin, T>> for stmt::Include {
    fn from(path: Path<Origin, T>) -> Self {
        stmt::Include::new(path.untyped)
    }
}
