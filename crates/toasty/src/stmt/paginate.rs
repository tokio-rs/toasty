use super::{List, Query};

use crate::{Executor, Result, schema::Load};

use toasty_core::stmt;

/// Cursor-based pagination over a [`Query`].
///
/// `Paginate` wraps a query with a fixed page size and provides
/// [`after`](Paginate::after) / [`before`](Paginate::before) methods for
/// forward and backward navigation using opaque cursors.
///
/// # Construction
///
/// Create a `Paginate` from a query via [`Paginate::new`] or by calling
/// `.into()` on a query that already has `limit` and `order_by` set.
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
/// use toasty::stmt::{List, Paginate, Query};
///
/// let mut q = Query::<List<User>>::all();
/// q.order_by(User::fields().name().asc());
/// let page = Paginate::new(q, 20)
///     .exec(&mut db)
///     .await
///     .unwrap();
/// # });
/// ```
///
/// # Requirements
///
/// The underlying query **must** have an `order_by` clause. [`Paginate::new`]
/// sets the limit for you; [`From<Query<M>>`] requires both `limit` and
/// `order_by` to be present already.
#[derive(Debug)]
pub struct Paginate<M> {
    query: Query<List<M>>,
    reverse: bool,
}

impl<M> Paginate<M> {
    /// Create a paginator from `query` with the given page size.
    ///
    /// The query must **not** already have a `limit` clause (this method sets
    /// it) and **must** have an `order_by` clause.
    ///
    /// # Panics
    ///
    /// Panics if `query` already has a `limit` or is missing `order_by`.
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
    /// use toasty::stmt::{List, Paginate, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.order_by(User::fields().name().asc());
    /// let _paginator = Paginate::new(q, 20);
    /// ```
    pub fn new(mut query: Query<List<M>>, per_page: usize) -> Self {
        assert!(
            query.untyped.limit.is_none(),
            "pagination requires no limit clause"
        );
        assert!(
            query.untyped.order_by.is_some(),
            "pagination requires an order_by clause"
        );

        query.untyped.limit = Some(stmt::Limit::Cursor(stmt::LimitCursor {
            page_size: stmt::Value::from(per_page as i64).into(),
            after: None,
        }));

        Self {
            query,
            reverse: false,
        }
    }

    /// Set the cursor for forward pagination.
    ///
    /// Records returned will come **after** `key` in the current sort order.
    /// Obtain `key` from [`Page::next_cursor`](super::Page::next_cursor) of a
    /// previous page.
    ///
    /// # Panics
    ///
    /// Panics if the query has no `limit` clause.
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
    /// use toasty::stmt::{List, Paginate, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.order_by(User::fields().id().asc());
    /// let paginator = Paginate::new(q, 10)
    ///     .after(toasty_core::stmt::Value::from(42_i64));
    /// ```
    pub fn after(mut self, key: impl Into<stmt::Expr>) -> Self {
        let Some(stmt::Limit::Cursor(cursor)) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a cursor limit clause");
        };
        cursor.after = Some(key.into());
        self.reverse = false;
        self
    }

    /// Set the cursor for backward pagination.
    ///
    /// Records returned will come **before** `key` in the current sort order.
    /// The result set is still returned in the original sort order (not
    /// reversed). Obtain `key` from
    /// [`Page::prev_cursor`](super::Page::prev_cursor) of a previous page.
    ///
    /// # Panics
    ///
    /// Panics if the query has no `limit` clause.
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
    /// use toasty::stmt::{List, Paginate, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.order_by(User::fields().id().asc());
    /// let paginator = Paginate::new(q, 10)
    ///     .before(toasty_core::stmt::Value::from(100_i64));
    /// ```
    pub fn before(mut self, key: impl Into<stmt::Expr>) -> Self {
        let Some(stmt::Limit::Cursor(cursor)) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a cursor limit clause");
        };
        cursor.after = Some(key.into());
        self.reverse = true;
        self
    }
}

impl<M: Load> Paginate<M> {
    /// Execute the paginated query and return a [`Page`](super::Page).
    ///
    /// The returned page contains up to `per_page` items along with optional
    /// cursors for the next and previous pages.
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
    /// use toasty::stmt::{List, Paginate, Query};
    ///
    /// let mut q = Query::<List<User>>::all();
    /// q.order_by(User::fields().name().asc());
    /// let page = Paginate::new(q, 20)
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn exec(mut self, executor: &mut dyn Executor) -> Result<super::Page<M::Output>> {
        let original_query = self.query.untyped.clone();

        // Reverse ORDER BY for backward pagination
        if self.reverse {
            let Some(order_by) = self.query.untyped.order_by.as_mut() else {
                panic!("pagination requires order by clause");
            };
            order_by.reverse();
        }

        // Execute with pagination - engine handles cursor extraction
        let response = executor
            .exec_untyped(stmt::Statement::Query(self.query.untyped.clone()))
            .await?;

        // Collect values from response
        let stmt::Value::List(mut items) = response.values.collect_as_value().await? else {
            return Err(crate::Error::invalid_result(
                "paginated query expected a list of rows",
            ));
        };

        // Reverse result set if paginating backward
        if self.reverse {
            items.reverse();
        }

        // Load the raw values into model instances
        let loaded_items: Vec<M::Output> = items.into_iter().map(M::load).collect::<Result<_>>()?;

        // For backward pagination, swap cursors (next becomes prev)
        let (next_cursor, prev_cursor) = if self.reverse {
            (response.prev_cursor, response.next_cursor)
        } else {
            (response.next_cursor, response.prev_cursor)
        };

        // Store the original query (not the reversed one) in the Page so that
        // subsequent .next() and .prev() calls use the correct ORDER BY direction
        Ok(super::Page::new(
            loaded_items,
            Query::from_untyped(original_query),
            next_cursor,
            prev_cursor,
        ))
    }
}

impl<M> From<Query<List<M>>> for Paginate<M> {
    fn from(value: Query<List<M>>) -> Self {
        assert!(
            value.untyped.limit.is_some(),
            "pagination requires a limit clause"
        );
        assert!(
            value.untyped.order_by.is_some(),
            "pagination requires an order_by clause"
        );

        Paginate {
            query: value,
            reverse: false,
        }
    }
}
