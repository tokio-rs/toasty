use super::{List, Query};

use crate::{engine::eval::Func, schema::Load, stmt::IntoStatement, Executor, Result};

use toasty_core::stmt::{self, visit_mut, Expr, ExprRecord, OrderBy, Projection, Value, VisitMut};

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

        query.untyped.limit = Some(stmt::Limit {
            limit: stmt::Value::from(per_page as i64).into(),
            offset: None,
        });

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
        let Some(limit) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a limit clause");
        };
        limit.offset = Some(stmt::Offset::After(key.into()));
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
        let Some(limit) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a limit clause");
        };
        limit.offset = Some(stmt::Offset::After(key.into()));
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
        // Extract the limit from the query to determine page size
        let page_size = match &self.query.untyped.limit {
            Some(stmt::Limit {
                limit: stmt::Expr::Value(stmt::Value::I64(n)),
                ..
            }) => *n as usize,
            _ => {
                let res = executor
                    .exec_untyped(self.query.clone().into_statement().untyped)
                    .await?;
                let stmt::Value::List(values) = res else {
                    todo!()
                };
                let items: Vec<M::Output> =
                    values.into_iter().map(M::load).collect::<Result<_>>()?;
                return Ok(super::Page::new(
                    items,
                    Query::from_untyped(self.query.untyped),
                    None,
                    None,
                ));
            }
        };

        // Query for one more item than requested to detect if there's a next page
        let mut query_with_extra = self.query.clone();
        if let Some(stmt::Limit { limit, .. }) = &mut query_with_extra.untyped.limit {
            *limit = stmt::Value::from((page_size + 1) as i64).into();
        }

        let Some(order_by) = query_with_extra.untyped.order_by.as_mut() else {
            panic!("pagination requires order by clause");
        };
        if self.reverse {
            order_by.reverse();
        }

        let res = executor
            .exec_untyped(query_with_extra.into_statement().untyped)
            .await?;

        let stmt::Value::List(mut items) = res else {
            todo!()
        };
        let has_next = (items.len() > page_size) || self.reverse;
        let has_prev = (items.len() > page_size) || !self.reverse;
        items.truncate(page_size);
        if self.reverse {
            items.reverse();
        }

        let Some(order_by) = self.query.untyped.order_by.as_mut() else {
            panic!("pagination requires order by clause");
        };
        // Create cursor from the first item for backwards pagination.
        let prev_cursor = match items.first() {
            Some(first_item) if has_prev => {
                extract_cursor(order_by, first_item).map(|cursor| cursor.into())
            }
            _ => None,
        };
        // Create cursor from the last item if there's a next for forwards page.
        let next_cursor = match items.last() {
            Some(last_item) if has_next => {
                extract_cursor(order_by, last_item).map(|cursor| cursor.into())
            }
            _ => None,
        };

        // Load the raw values into model instances
        let loaded_items: Vec<M::Output> = items.into_iter().map(M::load).collect::<Result<_>>()?;

        Ok(super::Page::new(
            loaded_items,
            Query::from_untyped(self.query.untyped),
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

/// Determines the next cursor of a paginated query from an [`OrderBy`] clause and an item [`Value`] in the result set.
fn extract_cursor(order_by: &OrderBy, item: &Value) -> Option<Value> {
    // Rewrite ExprReference::Field to ExprArg and pass the item value as the argument.
    let record = ExprRecord::from_iter(order_by.exprs.iter().map(|order_by_expr| {
        struct Visitor;
        impl VisitMut for Visitor {
            fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
                match i {
                    stmt::Expr::Reference(stmt::ExprReference::Field { nesting, index })
                        if *nesting == 0 =>
                    {
                        *i = Expr::arg_project(0, Projection::from_index(*index))
                    }
                    _ => visit_mut::visit_expr_mut(self, i),
                }
            }
        }

        let mut expr = order_by_expr.expr.clone();
        Visitor.visit_mut(&mut expr);
        expr
    }));
    Func::from_stmt(Expr::Record(record), vec![item.infer_ty()])
        .eval(&[item])
        .ok()
}
