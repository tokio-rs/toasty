use super::Select;

use crate::{cursor::FromCursor, Db, Model, Result};

use toasty_core::stmt::{self, Limit};

#[derive(Debug)]
pub struct Paginate<M> {
    /// How to query the data
    query: Select<M>,
}

impl<M: Model> Paginate<M> {
    pub fn new(mut query: Select<M>, per_page: usize) -> Self {
        assert!(
            query.untyped.limit.is_none(),
            "pagination requires no limit clause"
        );
        assert!(
            query.untyped.order_by.is_some(),
            "pagination requires an order_by clause"
        );

        query.untyped.limit = Some(stmt::Limit::PaginateForward {
            limit: stmt::Value::from(per_page as i64).into(),
            after: None,
        });

        Self { query }
    }

    /// Set the key-based offset for pagination.
    pub fn after(mut self, key: impl Into<stmt::Expr>) -> Self {
        let Some(Limit::PaginateForward { after, .. }) = self.query.untyped.limit.as_mut() else {
            panic!("expected cursor-based pagination in query");
        };

        *after = Some(key.into());

        self
    }

    pub async fn collect<A>(self, db: &Db) -> Result<A>
    where
        A: FromCursor<M>,
    {
        db.all(self.query).await?.collect().await
    }
}
