use std::marker::PhantomData;

use toasty_core::stmt;

use super::{Path, Query};

/// A typed wrapper around an untyped [`stmt::Include`] that carries the
/// origin model and the relation target type.
///
/// Produced by passing a relation path — optionally with a `.filter(...)`
/// predicate — to `.include(...)`.
pub struct Include<Origin, T> {
    pub(crate) untyped: stmt::Include,
    _p: PhantomData<(Origin, T)>,
}

impl<Origin, T> Include<Origin, T> {
    #[doc(hidden)]
    pub fn from_path_and_query<U>(path: Path<Origin, T>, query: Query<U>) -> Self {
        Self {
            untyped: stmt::Include::with_query(path.untyped, query.untyped),
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
