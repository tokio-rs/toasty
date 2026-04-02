mod association;
pub use association::Association;

mod batch;
pub use batch::{Batch, batch};

mod create_many;
pub use create_many::CreateMany;

mod delete;
pub use delete::Delete;

mod expr;
pub use expr::Expr;

mod insert;
pub use insert::Insert;

mod assignment;
pub use assignment::{Assign, Assignment, insert, remove, set};

mod into_expr;
pub use into_expr::IntoExpr;

mod into_insert;
pub use into_insert::IntoInsert;

mod into_statement;
pub use into_statement::IntoStatement;

mod list;
pub use list::List;

mod page;
pub use page::Page;

mod paginate;
pub use paginate::Paginate;

mod path;
pub use path::Path;

pub use crate::schema::Auto;
use crate::{Executor, schema::Load};

mod query;
pub use query::Query;

mod update;
pub use update::Update;

pub use toasty_core::stmt::{OrderBy, Projection, Value};

use toasty_core::stmt;

use std::{fmt, marker::PhantomData};

/// A typed wrapper around an untyped [`stmt::Statement`](toasty_core::stmt::Statement).
///
/// The type parameter `T` is the **returning type** — the type produced when the
/// statement is executed — not the model the statement operates on. For example,
/// a query that selects `User` records returns `List<User>`, so its statement
/// type is `Statement<List<User>>`, not `Statement<User>`.
///
/// Common returning types:
///
/// | Statement kind | `T` | `exec()` produces |
/// |---|---|---|
/// | Multi-row query | [`List<M>`] | `Vec<M>` |
/// | Single-row query (`.one()`) | `M` | `M` |
/// | Optional query (`.first()`) | `Option<M>` | `Option<M>` |
/// | Insert (create) | `M` | `M` |
/// | Delete | `()` | `()` |
/// | Update | `()` | `()` |
///
/// You rarely construct `Statement` directly. Instead, use the [`From`]
/// implementations to convert from [`Query`], [`Insert`], [`Update`], or
/// [`Delete`], or call [`IntoStatement::into_statement`] on a query builder.
pub struct Statement<T> {
    pub(crate) untyped: stmt::Statement,
    _p: PhantomData<T>,
}

impl<T> Statement<T> {
    /// Wrap a raw untyped [`stmt::Statement`](toasty_core::stmt::Statement),
    /// tagging it with returning type `T`.
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

    /// Consume this wrapper and return the underlying untyped statement.
    ///
    /// This is the inverse of [`from_untyped_stmt`](Self::from_untyped_stmt).
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty::stmt::Statement;
    /// # use toasty_core::stmt as core_stmt;
    /// let raw: core_stmt::Statement = core_stmt::Query::unit().into();
    /// let typed: Statement<()> = Statement::from_untyped_stmt(raw.clone());
    /// let back: core_stmt::Statement = typed.into_untyped();
    /// assert_eq!(raw, back);
    /// ```
    pub fn into_untyped(self) -> stmt::Statement {
        self.untyped
    }

    pub(crate) fn into_untyped_query(self) -> stmt::Query {
        match self.untyped {
            stmt::Statement::Query(q) => q,
            _ => panic!("expected query statement"),
        }
    }

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
    pub fn into_query(self) -> Option<Query<T>> {
        match self.untyped {
            stmt::Statement::Query(q) => Some(Query::from_untyped(q)),
            _ => None,
        }
    }
}

impl<T: Load> Statement<T> {
    /// Execute this statement against the given executor and return the
    /// deserialized result.
    ///
    /// This is a convenience wrapper around
    /// [`Executor::exec`](crate::Executor::exec).
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// # let mut db = toasty::Db::builder().models(toasty::models!(User)).build(driver).await.unwrap();
    /// # db.push_schema().await.unwrap();
    /// use toasty::stmt::{IntoStatement, List, Query};
    ///
    /// let stmt = Query::<List<User>>::all().into_statement();
    /// let users: Vec<User> = stmt.exec(&mut db).await.unwrap();
    /// # });
    /// ```
    pub async fn exec(self, executor: &mut dyn Executor) -> crate::Result<T::Output> {
        executor.exec(self).await
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
