use super::{Delete, Expr, IntoStatement, List, Statement, Value};
use crate::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt::{self, Offset};

pub struct Query<M> {
    /// How to filter the data source
    pub(crate) untyped: stmt::Query,

    _p: PhantomData<M>,
}

impl<M> Query<M> {
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

    // TODO: why are these by value?
    pub fn and(mut self, filter: Expr<bool>) -> Self {
        self.untyped.add_filter(filter.untyped);
        self
    }

    pub fn union(mut self, other: Self) -> Self {
        self.untyped.add_union(other.untyped);
        self
    }

    pub fn include(&mut self, path: impl Into<stmt::Path>) -> &mut Self {
        self.untyped.include(path.into());
        self
    }

    pub fn order_by(&mut self, order_by: impl Into<stmt::OrderBy>) -> &mut Self {
        self.untyped.order_by = Some(order_by.into());
        self
    }

    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.untyped.limit = Some(stmt::Limit {
            limit: stmt::Value::from(n as i64).into(),
            offset: None,
        });
        self
    }

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

    pub fn delete(self) -> Delete<M> {
        Delete::from_untyped(self.untyped.delete())
    }
}

impl<M: Model> Query<M> {
    pub fn filter(expr: Expr<bool>) -> Self {
        Self::from_untyped(stmt::Query::new_select(M::id(), expr.untyped))
    }

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
