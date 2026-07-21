use super::{Delete, Expr, IntoExpr, IntoStatement, List, Statement, Value};
use crate::{
    Executor, Result,
    schema::{Load, Model},
    stmt::Path,
};
use std::{fmt, marker::PhantomData};
use toasty_core::stmt::{self, Returning};

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
/// use [`Query::all`] and chain [`filter`](Query::filter):
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
/// let q = Query::<List<User>>::all().filter(User::fields().age().gt(18));
///
/// // Chained
/// let q = Query::<List<User>>::all()
///     .filter(User::fields().name().eq("Alice"))
///     .limit(10);
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

    /// Take the via association out of this query's source, if any.
    ///
    /// Returns `Some(_)` when the query was scoped from a relation traversal
    /// (e.g. built via [`Association::many`](crate::stmt::Association::many)).
    /// After the call the query no longer carries the association on its source.
    pub(crate) fn take_via_assoc(&mut self) -> Option<stmt::Association> {
        let stmt::ExprSet::Select(select) = &mut self.untyped.body else {
            return None;
        };
        let stmt::Source::Model(model) = &mut select.source else {
            return None;
        };
        model.via.take()
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
    ///     .filter(User::fields().name().eq("Alice"));
    /// ```
    pub fn filter(mut self, filter: Expr<bool>) -> Self {
        self.untyped.add_filter(filter.untyped);
        self
    }

    /// Sets the filter, combined with AND, for this query overwriting existing ones.
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
    /// q.set_filter(User::fields().name().eq("Alice"));
    /// ```
    pub fn set_filter(&mut self, filter: Expr<bool>) -> &mut Self {
        self.untyped.set_filter(filter);
        self
    }

    /// Eagerly load a related association when this query executes.
    ///
    /// `path` identifies the relation to include (e.g., a has-many or
    /// belongs-to field). The related records are loaded in the same
    /// round-trip and attached to the parent model.
    ///
    /// A multi-step (`via`) relation can also be included. Its targets are
    /// reached through the relation path and grouped under each parent, with
    /// duplicate targets collapsed so each one appears once. Including a `via`
    /// relation is supported on SQL backends (SQLite, PostgreSQL, MySQL); it
    /// is not yet available on DynamoDB.
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
    /// use toasty::schema::Model;
    ///
    /// // Include the field at index 1 (name)
    /// let q = Query::<List<User>>::all().include(User::path_field::<String>(1));
    /// ```
    pub fn include(mut self, include: impl Into<stmt::Include>) -> Self {
        self.untyped.include(include.into());
        self
    }

    /// Add a sort order to this query.
    ///
    /// Pass an [`OrderByExpr`](toasty_core::stmt::OrderByExpr) obtained from
    /// [`Path::asc`] or [`Path::desc`], or a tuple of them to sort by several
    /// fields at once. Calling `order_by` multiple times appends each
    /// expression to the existing order, so later calls act as tie-breakers
    /// for earlier ones.
    ///
    /// # Examples
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
    /// let q = Query::<List<User>>::all()
    ///     .order_by((User::fields().age().desc(), User::fields().name().asc()));
    /// ```
    pub fn order_by(mut self, order_by: impl Into<stmt::OrderBy>) -> Self {
        let order_by = order_by.into();
        match &mut self.untyped.order_by {
            Some(existing) => existing.exprs.extend(order_by.exprs),
            None => self.untyped.order_by = Some(order_by),
        }
        self
    }

    /// Sets the sort order for this query overwriting existing ones.
    ///
    /// Pass an [`OrderByExpr`](toasty_core::stmt::OrderByExpr) obtained from
    /// [`Path::asc`] or [`Path::desc`], or a tuple of them to sort by several
    /// fields at once. Calling `order_by` multiple times appends each
    /// expression to the existing order, so later calls act as tie-breakers
    /// for earlier ones.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// #     age: i64
    /// # }
    /// use toasty::stmt::{List, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.set_order_by(User::fields().name().desc());
    /// ```
    pub fn set_order_by(&mut self, order_by: impl Into<stmt::OrderBy>) -> &mut Self {
        let order_by = order_by.into();
        self.untyped.order_by = Some(order_by);
        self
    }

    /// Limit the number of records returned.
    ///
    /// `n` is an upper bound, not a guarantee. The limit is applied to the
    /// database query, but Toasty may apply additional filtering to the rows
    /// the database returns. When that happens, the final result can have
    /// fewer than `n` records even if more than `n` rows match the filter
    /// expression.
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
    /// let q = Query::<List<User>>::all().limit(10);
    /// ```
    pub fn limit(mut self, n: usize) -> Self {
        let n = i64::try_from(n).expect("limit exceeds i64::MAX");
        self.untyped.limit = Some(stmt::Limit::Offset(stmt::LimitOffset {
            limit: stmt::Value::from(n).into(),
            offset: None,
        }));
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
    /// let q = Query::<List<User>>::all().limit(10).offset(20);
    /// ```
    pub fn offset(mut self, n: usize) -> Self {
        let n = i64::try_from(n).expect("offset exceeds i64::MAX");
        self.untyped.limit = match self.untyped.limit.take() {
            Some(stmt::Limit::Offset(limit_offset)) => {
                Some(stmt::Limit::Offset(stmt::LimitOffset {
                    limit: limit_offset.limit,
                    offset: Some(stmt::Value::from(n).into()),
                }))
            }
            Some(stmt::Limit::Cursor(_)) => {
                panic!("cannot use offset with cursor-based pagination")
            }
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
    /// let delete = Query::<List<User>>::all().filter(User::fields().name().eq("Alice"))
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
    /// This applies `LIMIT 1` to the database query. Toasty may then filter
    /// the returned row, so `None` does not always mean that no rows in the
    /// table match the filter expression.
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
    /// This applies `LIMIT 1` to the database query. If Toasty filters the
    /// returned row out, execution returns the same error as when the database
    /// returns no rows.
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
    query.limit = Some(stmt::Limit::Offset(stmt::LimitOffset {
        limit: stmt::Expr::Static(stmt::Value::I64(1)),
        offset: None,
    }));
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
    /// # let mut db = toasty::Db::builder().models(toasty::models!(User)).build(driver).await.unwrap();
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
    /// Convert this list query into a count query that returns the number of
    /// matching records as a `u64`.
    ///
    /// The returned `Query<u64>` wraps a `SELECT COUNT(*)` query.
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
    /// let q: Query<u64> = Query::<List<User>>::all().count();
    /// ```
    pub fn count(mut self) -> Query<u64> {
        // Set the returning clause to COUNT(*)
        *self.untyped.returning_mut_unwrap() = Returning::Project(stmt::Expr::count_star());
        self.untyped.single = true;

        Query::from_untyped(self.untyped)
    }

    /// Project this list query onto an expression, narrowing the returned
    /// shape from `M` to `T`.
    ///
    /// `projection` can be any expression source: a single field path
    /// (returning `Vec<T>` for the field's Rust type), a tuple of field paths
    /// (returning `Vec` of a tuple), or any other type that implements
    /// `IntoExpr<T>`.  The default model projection is replaced wholesale by
    /// the columns the projection expression references.
    ///
    /// A multi-step (`via`) relation can be projected as well: a `has_many`
    /// `via` yields a `Vec` of the distinct targets reached through the path
    /// per row, and a single (`has_one`) `via` yields one target (or `None`).
    /// This is supported on SQL backends (SQLite, PostgreSQL, MySQL); it is
    /// not yet available on DynamoDB.
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
    /// use toasty::stmt::{List, Query};
    ///
    /// // Single-field projection: returns `Vec<String>`.
    /// let names: Vec<String> = Query::<List<User>>::all()
    ///     .select(User::fields().name())
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    ///
    /// // Tuple projection: returns `Vec<(i64, String)>`.
    /// let pairs: Vec<(i64, String)> = Query::<List<User>>::all()
    ///     .select((User::fields().id(), User::fields().name()))
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub fn select<E, T>(mut self, projection: E) -> Query<List<T>>
    where
        E: IntoExpr<T>,
        T: Load,
    {
        *self.untyped.returning_mut_unwrap() = Returning::Project(projection.into_expr().untyped);

        Query::from_untyped(self.untyped)
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

    /// Sort this query so the most recently inserted records on `field`
    /// appear first.
    ///
    /// Convenience for [`order_by`](Self::order_by) with [`Path::desc`].
    /// `field` must be a path rooted at this query's model `M`.
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
    /// let q = Query::<List<User>>::all().latest_by(User::fields().id());
    /// ```
    pub fn latest_by<U>(self, field: Path<M, U>) -> Self {
        self.order_by(field.desc())
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
