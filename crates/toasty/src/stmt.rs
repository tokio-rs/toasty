mod association;
pub use association::Association;

mod delete;
pub use delete::Delete;

mod expr;
pub use expr::Expr;

mod insert;
pub use insert::Insert;

mod into_expr;
pub use into_expr::IntoExpr;

mod into_insert;
pub use into_insert::IntoInsert;

mod into_statement;
pub use into_statement::IntoStatement;

mod list;
pub use list::List;

mod paginate;
pub use paginate::Paginate;

mod path;
pub use path::Path;

pub use crate::schema::{Auto, Field};

mod query;
pub use query::Query;

mod update;
pub use update::Update;

pub use toasty_core::stmt::{OrderBy, Projection, Value};

use toasty_core::stmt;

use std::{fmt, marker::PhantomData};

pub struct Statement<M> {
    pub(crate) untyped: stmt::Statement,
    _p: PhantomData<M>,
}

impl<M> Statement<M> {
    /// Wrap a raw untyped statement.
    pub fn from_untyped_stmt(untyped: stmt::Statement) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    pub(crate) fn into_untyped_query(self) -> stmt::Query {
        match self.untyped {
            stmt::Statement::Query(q) => q,
            _ => panic!("expected query statement"),
        }
    }
}

impl<M> Statement<List<M>> {
    pub fn into_query(self) -> Option<Query<M>> {
        match self.untyped {
            stmt::Statement::Query(q) => Some(Query::from_untyped(q)),
            _ => None,
        }
    }
}

impl<M> From<Query<M>> for Statement<M> {
    fn from(value: Query<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Insert<M>> for Statement<M> {
    fn from(value: Insert<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Update<M>> for Statement<M> {
    fn from(value: Update<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Statement<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}

/// Returns an expression that tests whether `lhs` is contained in `rhs`.
///
/// This works for both single fields and tuples of fields (composite keys):
///
/// ```ignore
/// // Single field
/// toasty::stmt::in_list(User::fields().id(), &ids)
///
/// // Composite key
/// toasty::stmt::in_list(
///     (Foo::fields().one(), Foo::fields().two()),
///     [("a", "b"), ("c", "d")],
/// )
/// ```
pub fn in_list<T>(lhs: impl IntoExpr<T>, rhs: impl IntoExpr<List<T>>) -> Expr<bool> {
    Expr::from_untyped(stmt::Expr::in_list(
        lhs.into_expr().untyped,
        rhs.into_expr().untyped,
    ))
}
