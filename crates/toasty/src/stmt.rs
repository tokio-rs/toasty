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

/// A typed wrapper around an untyped [`stmt::Statement`](toasty_core::stmt::Statement).
///
/// `Statement<M>` pairs a raw statement AST node with a type `M` that tracks
/// what the statement returns when executed. For example:
///
/// - `Statement<List<User>>` — a query returning a collection of `User` records.
/// - `Statement<User>` — an insert returning the newly created `User`.
/// - `Statement<()>` — a delete returning nothing.
///
/// You rarely construct `Statement` directly. Instead, use the [`From`]
/// implementations to convert from [`Query`], [`Insert`], [`Update`], or
/// [`Delete`], or call [`IntoStatement::into_statement`] on a query builder.
pub struct Statement<M> {
    pub(crate) untyped: stmt::Statement,
    _p: PhantomData<M>,
}

impl<M> Statement<M> {
    /// Wrap a raw untyped [`stmt::Statement`](toasty_core::stmt::Statement),
    /// tagging it with type `M`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty::stmt::Statement;
    /// # use toasty_core::stmt as core_stmt;
    /// let raw: core_stmt::Statement = core_stmt::Query::unit().into();
    /// let _typed: Statement<()> = Statement::from_untyped_stmt(raw);
    /// ```
    pub fn from_untyped_stmt(untyped: stmt::Statement) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    /// Change the type tag of this statement without altering the underlying
    /// untyped representation.
    pub fn cast<T>(self) -> Statement<T> {
        Statement {
            untyped: self.untyped,
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
    /// Try to extract the inner [`Query`] from this statement.
    ///
    /// Returns `Some(query)` if the statement is a query, or `None` for
    /// inserts, updates, and deletes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty::stmt::{Query, Statement, List};
    /// # use toasty_core::stmt as core_stmt;
    /// let query_stmt: Statement<List<()>> = Statement::from_untyped_stmt(
    ///     core_stmt::Query::unit().into(),
    /// );
    /// assert!(query_stmt.into_query().is_some());
    /// ```
    pub fn into_query(self) -> Option<Query<List<M>>> {
        match self.untyped {
            stmt::Statement::Query(q) => Some(Query::from_untyped(q)),
            _ => None,
        }
    }
}

impl<M> Statement<M> {
    /// Try to extract the inner [`Query`] as a list query from this statement.
    ///
    /// This is useful when a `Statement<M>` (single-model) wraps an underlying
    /// list query (e.g., a model instance filtered by primary key). The type
    /// parameter is re-wrapped as `List<M>` since the underlying query is a
    /// select.
    pub fn into_list_query(self) -> Option<Query<List<M>>> {
        match self.untyped {
            stmt::Statement::Query(q) => Some(Query::from_untyped(q)),
            _ => None,
        }
    }
}

impl<M> From<Query<List<M>>> for Statement<List<M>> {
    fn from(value: Query<List<M>>) -> Self {
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
/// # Examples
///
/// ```
/// # #[derive(Debug, toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// # }
/// // Single field — test if a user's id is in a list
/// let filter = toasty::stmt::in_list(User::fields().id(), [1_i64, 2, 3]);
/// ```
pub fn in_list<T>(lhs: impl IntoExpr<T>, rhs: impl IntoExpr<List<T>>) -> Expr<bool> {
    Expr::from_untyped(stmt::Expr::in_list(
        lhs.into_expr().untyped,
        rhs.into_expr().untyped,
    ))
}
