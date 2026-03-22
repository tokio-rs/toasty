use super::{Delete, Expr, IntoStatement, List, Statement, Value};
use crate::{
    schema::{Load, Model},
    Executor, Result,
};
use std::{fmt, marker::PhantomData};
use toasty_core::stmt::{self, Offset};

/// A typed query that selects records from the database.
///
/// The type parameter `T` is the **returning type** — it encodes what
/// `exec()` produces, not just which model is being queried. A `Query` starts
/// as `Query<List<M>>` (returns `Vec<M>`) and can be narrowed:
///
/// | Type | `exec()` produces | Created by |
/// |---|---|---|
/// | `Query<List<M>>` | `Vec<M>` | [`Query::all`], [`Query::filter`] |
/// | `Query<M>` | `M` (errors if missing) | [`.one()`](Query::one) |
/// | `Query<Option<M>>` | `Option<M>` | [`.first()`](Query::first) |
///
/// # Building queries
///
/// Start with a generated finder (e.g., `User::filter_by_name("Alice")`) or
/// use [`Query::all`] / [`Query::filter`] directly:
///
/// ```
/// # #[derive(Debug, toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// # }
/// use toasty::stmt::{List, Query};
///
/// // All users
/// let q = Query::<List<User>>::all();
///
/// // Filtered
/// let q = Query::<List<User>>::filter(User::fields().age().gt(18));
///
/// // Chained
/// let mut q = Query::<List<User>>::all()
///     .and(User::fields().name().eq("Alice"));
/// q.limit(10);
/// ```
///
/// # Execution
///
/// Pass the query to [`Db::exec`](crate::Db::exec) or convert it with
/// [`IntoStatement`] for batch use.
pub struct Query<T> {
    pub(crate) untyped: stmt::Query,
    _p: PhantomData<T>,
}

// Methods available on all Query<T>
impl<T> Query<T> {
    /// Create an empty unit query that returns no records.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty::stmt::Query;
    /// let q = Query::<()>::unit();
    /// ```
    pub fn unit() -> Self {
        Self {
            untyped: stmt::Query::unit(),
            _p: PhantomData,
        }
    }

    pub(crate) const fn from_untyped(untyped: stmt::Query) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    /// Convert a model expression to a query.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty::stmt::{Query, Expr};
    /// # use toasty_core::stmt as core_stmt;
    /// let expr = Expr::<i64>::from_untyped(core_stmt::Expr::Value(
    ///     core_stmt::Value::from(42_i64),
    /// ));
    /// let _q = Query::from_expr(expr);
    /// ```
    pub fn from_expr(expr: Expr<T>) -> Self {
        match expr.untyped {
            stmt::Expr::Stmt(expr) => match *expr.stmt {
                stmt::Statement::Query(stmt) => Self::from_untyped(stmt),
                stmt => todo!("stmt={stmt:#?}"),
            },
            expr => Self::from_untyped(stmt::Query::values(expr)),
        }
    }

    /// Add an additional filter, combined with AND, to this query.
    ///
    /// Returns `self` for method chaining.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let q = Query::<List<User>>::all()
    ///     .and(User::fields().name().eq("Alice"));
    /// ```
    pub fn and(mut self, filter: Expr<bool>) -> Self {
        self.untyped.add_filter(filter.untyped);
        self
    }

    /// Eagerly load a related association when this query executes.
    ///
    /// `path` identifies the relation to include (e.g., a has-many or
    /// belongs-to field). The related records are loaded in the same
    /// round-trip and attached to the parent model.
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
    /// use toasty::stmt::{List, Path, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// // Include the field at index 1 (name)
    /// q.include(Path::<User, String>::from_field_index(1));
    /// ```
    pub fn include(&mut self, path: impl Into<stmt::Path>) -> &mut Self {
        self.untyped.include(path.into());
        self
    }

    /// Set the sort order for this query.
    ///
    /// Pass an [`OrderByExpr`](toasty_core::stmt::OrderByExpr) obtained from
    /// [`Path::asc`] or [`Path::desc`]:
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.order_by(User::fields().name().desc());
    /// ```
    pub fn order_by(&mut self, order_by: impl Into<stmt::OrderBy>) -> &mut Self {
        self.untyped.order_by = Some(order_by.into());
        self
    }

    /// Limit the number of records returned.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.limit(10);
    /// ```
    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.untyped.limit = Some(stmt::Limit {
            limit: stmt::Value::from(n as i64).into(),
            offset: None,
        });
        self
    }

    /// Skip the first `n` records. Requires a prior call to [`limit`](Query::limit).
    ///
    /// # Panics
    ///
    /// Panics if no `limit` has been set on this query.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.limit(10);
    /// q.offset(20);
    /// ```
    pub fn offset(&mut self, n: usize) -> &mut Self {
        self.untyped.limit = match self.untyped.limit.take() {
            Some(limit) => Some(stmt::Limit {
                limit: limit.limit,
                offset: Some(Offset::Count(stmt::Expr::Value(Value::from(n)))),
            }),
            None => panic!("limit required for offset"),
        };
        self
    }

    /// Convert this query into a [`Delete`] statement that removes all matching
    /// records.
    ///
    /// The returned `Delete<()>` does not return the removed records.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let delete = Query::<List<User>>::filter(User::fields().name().eq("Alice"))
    ///     .delete();
    /// ```
    pub fn delete(self) -> Delete<()> {
        Delete::from_untyped(self.untyped.delete())
    }

    /// Widen a single-row query back into a list query.
    ///
    /// This is the inverse of [`first`](Query::first) or [`one`](Query::one).
    /// Panics if the query is not currently in single-row mode.
    ///
    /// # Panics
    ///
    /// Panics if this query is not a single-row query.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let q: Query<User> = Query::<List<User>>::all().one();
    /// let _list_q: Query<List<User>> = q.to_list();
    /// ```
    pub fn to_list(mut self) -> Query<List<T>> {
        assert!(self.untyped.single, "not a single query");
        self.untyped.single = false;

        Query {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }
}

impl<T> Query<List<T>> {
    /// Narrow this list query to return at most one record, wrapped in
    /// `Option`.
    ///
    /// The resulting `Query<Option<T>>` returns `Some(record)` if at least one
    /// row matches, or `None` if no rows match.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let q: Query<Option<User>> = Query::<List<User>>::all().first();
    /// ```
    pub fn first(mut self) -> Query<Option<T>> {
        set_first(&mut self.untyped);

        Query {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }

    /// Narrow this list query to return exactly one record.
    ///
    /// The resulting `Query<T>` returns the record directly. If no rows match,
    /// execution returns an error.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let q: Query<User> = Query::<List<User>>::all().one();
    /// ```
    pub fn one(mut self) -> Query<T> {
        set_first(&mut self.untyped);

        Query {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }
}

fn set_first(query: &mut stmt::Query) {
    assert!(!query.single, "query is single");
    query.single = true;
}

impl<T: Load> Query<T> {
    /// Execute this query against the given executor and return the result.
    ///
    /// The return type depends on the query's type parameter `T`:
    /// - `Query<List<M>>` returns `Vec<M>`.
    /// - `Query<M>` returns `M` (errors if no row matches).
    /// - `Query<Option<M>>` returns `Option<M>`.
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
    /// # let mut db = toasty::Db::builder().register::<User>().build(driver).await.unwrap();
    /// # db.push_schema().await.unwrap();
    /// use toasty::stmt::{List, Query};
    ///
    /// let users: Vec<User> = Query::<List<User>>::all()
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T::Output> {
        executor.exec(self.into_statement()).await
    }
}

/// Methods for list queries: `Query<List<M>>`
impl<M: Model> Query<List<M>> {
    /// Create a query that selects records of `M` matching `expr`.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let q = Query::<List<User>>::filter(User::fields().name().eq("Alice"));
    /// ```
    pub fn filter(expr: Expr<bool>) -> Self {
        let mut query = stmt::Query::new_select(M::id(), expr.untyped);
        query.single = false;
        Self::from_untyped(query)
    }

    /// Create a query that selects all records of `M`.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// let q = Query::<List<User>>::all();
    /// ```
    pub fn all() -> Self {
        let filter = stmt::Expr::Value(Value::from_bool(true));
        let mut query = stmt::Query::new_select(M::id(), filter);
        query.single = false;
        Self::from_untyped(query)
    }
}

impl<T> IntoStatement for Query<T> {
    type Returning = T;

    fn into_statement(self) -> Statement<T> {
        Statement::from_untyped_stmt(self.untyped.into())
    }
}

impl<T> IntoStatement for &Query<T> {
    type Returning = T;

    fn into_statement(self) -> Statement<T> {
        Statement::from_untyped_stmt(self.clone().untyped.into())
    }
}

impl<T> Clone for Query<T> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<T> fmt::Debug for Query<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
