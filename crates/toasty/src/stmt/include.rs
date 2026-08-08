use std::marker::PhantomData;

use toasty_core::stmt;

use super::{Expr, Path, Query};

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

impl<Origin, T> Include<Origin, T> {
    /// Restricts the related rows loaded by this include.
    pub fn filter(mut self, predicate: Expr<bool>) -> Self {
        self.query_mut().add_filter(predicate.untyped);
        self
    }

    /// Orders the related rows loaded by this include.
    pub fn order_by(mut self, order_by: impl Into<stmt::OrderBy>) -> Self {
        let order_by = order_by.into();
        match &mut self.query_mut().order_by {
            Some(existing) => existing.exprs.extend(order_by.exprs),
            slot @ None => *slot = Some(order_by),
        }
        self
    }

    fn query_mut(&mut self) -> &mut stmt::Query {
        self.untyped
            .query
            .as_mut()
            .expect("include modifiers require a related-model query")
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
