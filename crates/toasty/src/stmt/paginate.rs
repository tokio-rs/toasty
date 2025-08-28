use super::Select;

use crate::{Db, Model, Result};

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

        query.untyped.limit = Some(stmt::Limit::PaginateForward {
            limit: stmt::Value::from(per_page as i64).into(),
            after: None,
        });

        Self { query }
    }

    /// Set the key-based offset for pagination.
    pub fn after(mut self, key: impl Into<stmt::Expr>) -> Self {
        let Some(limit) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a limit clause");
        };

        match limit {
            stmt::Limit::PaginateForward { after, .. } => {
                *after = Some(key.into());
            }
            stmt::Limit::Offset { .. } => {
                panic!("Cannot set after cursor on offset-based limit");
            }
        }

        self
    }

    pub async fn collect(self, db: &Db) -> Result<crate::Page<M>> {
        // Extract the limit from the query to determine page size
        let page_size = match &self.query.untyped.limit {
            Some(stmt::Limit::PaginateForward { limit, .. }) => {
                match limit {
                    stmt::Expr::Value(stmt::Value::I64(n)) => *n as usize,
                    _ => {
                        // Fallback if we can't determine the limit
                        let items: Vec<M> = db.all(self.query.clone()).await?.collect().await?;
                        return Ok(crate::Page::new(items, self.query, None, None));
                    }
                }
            }
            _ => {
                // Not a paginated query, just collect all items
                let items: Vec<M> = db.all(self.query.clone()).await?.collect().await?;
                return Ok(crate::Page::new(items, self.query, None, None));
            }
        };

        // Query for one more item than requested to detect if there's a next page
        let mut query_with_extra = self.query.clone();
        if let Some(stmt::Limit::PaginateForward { limit, .. }) =
            &mut query_with_extra.untyped.limit
        {
            *limit = stmt::Value::from((page_size + 1) as i64).into();
        }

        let items: Vec<M> = db.all(query_with_extra).await?.collect().await?;

        let has_next = items.len() > page_size;
        let items = if has_next {
            items.into_iter().take(page_size).collect()
        } else {
            items
        };

        // Create cursor from the last item if there's a next page
        let next_cursor = if has_next && !items.is_empty() {
            // TODO: Implement proper cursor extraction from the last item
            // For now, use a placeholder cursor value to indicate there's a next page
            Some(stmt::Value::from(0_i64).into())
        } else {
            None
        };

        Ok(crate::Page::new(
            items,
            self.query,
            next_cursor,
            None, // prev_cursor not implemented yet
        ))
    }
}
