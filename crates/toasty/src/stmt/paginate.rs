use super::Select;

use crate::{cursor::FromCursor, Db, Model, Result};

use toasty_core::stmt;

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

        query.untyped.limit = Some(stmt::Limit {
            limit: stmt::Value::from(per_page as i64).into(),
            offset: None,
        });

        Self { query }
    }

    pub async fn collect<A>(self, db: &Db) -> Result<A>
    where
        A: FromCursor<M>,
    {
        db.all(self.query).await?.collect().await
    }
}
