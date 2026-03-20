use super::Query;

use crate::{Executor, Load, Result};

use toasty_core::stmt::{self, Value};

#[derive(Debug)]
pub struct Paginate<M> {
    /// How to query the data
    query: Query<M>,

    /// Whether we are currently paginating backwards.
    ///
    /// Because the sort order has to be reversed during backwards pagination,
    /// we need to reverse the result set again to go back to the expected order.
    reverse: bool,
}

impl<M> Paginate<M> {
    pub fn new(mut query: Query<M>, per_page: usize) -> Self {
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

    /// Set the key-based offset for forwards pagination.
    pub fn after(mut self, key: impl Into<stmt::Expr>) -> Self {
        let Some(limit) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a limit clause");
        };
        limit.offset = Some(stmt::Offset::After(key.into()));
        self.reverse = false;
        self
    }

    /// Set the key-based offset for backwards pagination.
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
    pub async fn exec(mut self, executor: &mut dyn Executor) -> Result<crate::Page<M::Output>> {
        // Save the original query before potentially modifying it for execution
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
            .exec_paginated(stmt::Statement::Query(self.query.untyped.clone()))
            .await?;

        // Collect values from response
        let mut items: Vec<Value> = response.values.into_value_stream().collect().await?;

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
        Ok(crate::Page::new(
            loaded_items,
            Query::from_untyped(original_query),
            next_cursor,
            prev_cursor,
        ))
    }
}

impl<M> From<Query<M>> for Paginate<M> {
    fn from(value: Query<M>) -> Self {
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
