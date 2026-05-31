use super::{Expr, IntoStatement, List, Path, Query, Statement};
use crate::{Executor, Result, schema::Load};
use std::fmt;
use toasty_core::stmt::{self, Returning};

/// Query wrapper for a projected multi-step relation.
#[doc(hidden)]
pub struct ProjectedMany<T> {
    query: Query<List<T>>,
}

/// Query wrapper for a projected single multi-step relation.
#[doc(hidden)]
pub struct ProjectedOne<T> {
    query: Query<T>,
}

impl<T> ProjectedMany<T> {
    pub(crate) fn from_association<Source, Target>(
        source: Query<List<Source>>,
        path: Path<Source, Target>,
    ) -> Self
    where
        Source: crate::schema::Model,
    {
        let query = association_query(source, path, false);

        Self {
            query: Query::from_untyped(query),
        }
    }

    pub fn filter(mut self, filter: Expr<bool>) -> Self {
        self.query = self.query.and(filter);
        self
    }

    pub fn order_by(mut self, order_by: impl Into<stmt::OrderBy>) -> Self {
        self.query.order_by(order_by);
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.query.limit(n);
        self
    }

    pub fn offset(mut self, n: usize) -> Self {
        self.query.offset(n);
        self
    }
}

impl<T: Load> ProjectedMany<T> {
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<Vec<T::Output>> {
        self.query.exec(executor).await
    }
}

impl<T> IntoStatement for ProjectedMany<T> {
    type Returning = List<T>;

    fn into_statement(self) -> Statement<List<T>> {
        self.query.into_statement()
    }
}

impl<T> Clone for ProjectedMany<T> {
    fn clone(&self) -> Self {
        Self {
            query: self.query.clone(),
        }
    }
}

impl<T> fmt::Debug for ProjectedMany<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.query.fmt(fmt)
    }
}

impl<T> ProjectedOne<T> {
    pub(crate) fn from_association<Source, Target>(
        source: Query<List<Source>>,
        path: Path<Source, Target>,
    ) -> Self
    where
        Source: crate::schema::Model,
    {
        let query = association_query(source, path, true);

        Self {
            query: Query::from_untyped(query),
        }
    }

    pub fn filter(mut self, filter: Expr<bool>) -> Self {
        self.query = self.query.and(filter);
        self
    }

    pub fn order_by(mut self, order_by: impl Into<stmt::OrderBy>) -> Self {
        self.query.order_by(order_by);
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.query.limit(n);
        self
    }

    pub fn offset(mut self, n: usize) -> Self {
        self.query.offset(n);
        self
    }
}

impl<T: Load> ProjectedOne<T> {
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T::Output> {
        self.query.exec(executor).await
    }
}

impl<T> IntoStatement for ProjectedOne<T> {
    type Returning = T;

    fn into_statement(self) -> Statement<T> {
        self.query.into_statement()
    }
}

impl<T> Clone for ProjectedOne<T> {
    fn clone(&self) -> Self {
        Self {
            query: self.query.clone(),
        }
    }
}

impl<T> fmt::Debug for ProjectedOne<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.query.fmt(fmt)
    }
}

fn association_query<Source, Target>(
    source: Query<List<Source>>,
    path: Path<Source, Target>,
    single: bool,
) -> stmt::Query
where
    Source: crate::schema::Model,
{
    let path: stmt::Path = path.into();
    assert_eq!(path.root.as_model_unwrap(), Source::id());

    let mut query = stmt::Query::builder(stmt::SourceModel::unresolved_via(stmt::Association {
        source: Box::new(source.untyped),
        path,
    }))
    .returning(Returning::Model { include: vec![] })
    .build();

    if single {
        query.single = true;
        query.limit = Some(stmt::Limit::Offset(stmt::LimitOffset {
            limit: stmt::Value::from(1_i64).into(),
            offset: None,
        }));
    }

    query
}

impl<T> From<ProjectedMany<T>> for Query<List<T>> {
    fn from(value: ProjectedMany<T>) -> Self {
        value.query
    }
}

impl<T> From<ProjectedOne<T>> for Query<T> {
    fn from(value: ProjectedOne<T>) -> Self {
        value.query
    }
}
