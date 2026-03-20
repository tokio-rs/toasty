use super::{Delete, Expr, IntoStatement, List, Statement, Value};
use crate::schema::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt::{self, Offset};

/// A typed query that selects records of model `M`.
///
/// `Query` is the main builder for read operations. It wraps an untyped
/// [`stmt::Query`](toasty_core::stmt::Query) and provides methods to add
/// filters, ordering, limits, and includes.
///
/// # Building queries
///
/// Start with a generated finder (e.g., `User::find_by_name("Alice")`) or
/// use [`Query::all`] / [`Query::filter`] directly:
///
/// ```ignore
/// // All users
/// let q = User::all();
///
/// // Filtered
/// let q = User::filter(User::fields().age().gt(18));
///
/// // Chained
/// let q = User::all()
///     .and(User::fields().name().eq("Alice"))
///     .limit(10);
/// ```
///
/// # Execution
///
/// Pass the query to [`Db::exec`](crate::Db::exec) or convert it with
/// [`IntoStatement`] for batch use.
pub struct Query<M> {
    pub(crate) untyped: stmt::Query,
    _p: PhantomData<M>,
}

impl<M> Query<M> {
    /// Create an empty unit query that returns no records.
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

    /// Convert a model expression to a query
    pub fn from_expr(expr: Expr<M>) -> Self {
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
    pub fn and(mut self, filter: Expr<bool>) -> Self {
        self.untyped.add_filter(filter.untyped);
        self
    }

    /// Combine this query with `other` using UNION, returning records that
    /// match either query.
    pub fn union(mut self, other: Self) -> Self {
        self.untyped.add_union(other.untyped);
        self
    }

    /// Eagerly load a related association when this query executes.
    ///
    /// `path` identifies the relation to include (e.g., a has-many or
    /// belongs-to field). The related records are loaded in the same
    /// round-trip and attached to the parent model.
    pub fn include(&mut self, path: impl Into<stmt::Path>) -> &mut Self {
        self.untyped.include(path.into());
        self
    }

    /// Set the sort order for this query.
    ///
    /// Pass an [`OrderByExpr`](toasty_core::stmt::OrderByExpr) obtained from
    /// [`Path::asc`] or [`Path::desc`]:
    ///
    /// ```ignore
    /// query.order_by(User::fields().created_at().desc());
    /// ```
    pub fn order_by(&mut self, order_by: impl Into<stmt::OrderBy>) -> &mut Self {
        self.untyped.order_by = Some(order_by.into());
        self
    }

    /// Limit the number of records returned.
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

    /// Convert this query into a [`Delete`] that removes all matching records.
    pub fn delete(self) -> Delete<M> {
        Delete::from_untyped(self.untyped.delete())
    }
}

impl<M: Model> Query<M> {
    /// Create a query that selects records of `M` matching `expr`.
    pub fn filter(expr: Expr<bool>) -> Self {
        Self::from_untyped(stmt::Query::new_select(M::id(), expr.untyped))
    }

    /// Create a query that selects all records of `M`.
    pub fn all() -> Self {
        let filter = stmt::Expr::Value(Value::from_bool(true));
        Self::from_untyped(stmt::Query::new_select(M::id(), filter))
    }
}

impl<M: Model> IntoStatement for Query<M> {
    type Returning = List<M>;

    fn into_statement(self) -> Statement<List<M>> {
        Statement::from_untyped_stmt(self.untyped.into())
    }
}

impl<M: Model> IntoStatement for &Query<M> {
    type Returning = List<M>;

    fn into_statement(self) -> Statement<List<M>> {
        Statement::from_untyped_stmt(self.clone().untyped.into())
    }
}

impl<M> Clone for Query<M> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Query<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
