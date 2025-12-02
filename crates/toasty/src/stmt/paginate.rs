use super::Select;

use crate::{engine::eval::Func, Db, Model, Result};

use toasty_core::stmt::{self, visit_mut, Expr, ExprRecord, OrderBy, Projection, Value, VisitMut};

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

    /// Set the key-based offset for pagination.
    pub fn after(mut self, key: impl Into<stmt::Expr>) -> Self {
        let Some(limit) = self.query.untyped.limit.as_mut() else {
            panic!("pagination requires a limit clause");
        };

        limit.offset = Some(stmt::Offset::After(key.into()));

        self
    }

    pub async fn collect(self, db: &Db) -> Result<crate::Page<M>> {
        // Extract the limit from the query to determine page size
        let page_size = match &self.query.untyped.limit {
            Some(stmt::Limit { limit, .. }) => {
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
        if let Some(stmt::Limit { limit, .. }) = &mut query_with_extra.untyped.limit {
            *limit = stmt::Value::from((page_size + 1) as i64).into();
        }

        let mut items: Vec<M> = db.all(query_with_extra).await?.collect().await?;
        let has_next = items.len() > page_size;
        items.truncate(page_size);

        // Create cursor from the last item if there's a next page
        let next_cursor = match items.last() {
            Some(last_item) if has_next => {
                dbg!(&self.query.untyped.order_by);
                let kek = 5;
                let cursor_expr = extract_cursor(
                    self.query
                        .untyped
                        .order_by
                        .as_ref()
                        .expect("pagination requires order by clause"),
                    last_item,
                );
                dbg!(&cursor_expr);

                Some(stmt::Value::from(0_i64).into())
            }
            _ => None,
        };

        Ok(crate::Page::new(
            items,
            self.query,
            next_cursor,
            None, // prev_cursor not implemented yet
        ))
    }
}

impl<M> From<Select<M>> for Paginate<M> {
    fn from(value: Select<M>) -> Self {
        assert!(
            value.untyped.limit.is_some(),
            "pagination requires a limit clause"
        );
        assert!(
            value.untyped.order_by.is_some(),
            "pagination requires an order_by clause"
        );

        Paginate { query: value }
    }
}

fn extract_cursor<M: Model>(order_by: &OrderBy, item: &M) -> Value {
    let record = ExprRecord::from_iter(order_by.exprs.iter().map(|order_by_expr| {
        struct Visitor {}

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
        Visitor {}.visit_mut(&mut expr);
        expr
    }));
    Func::from_stmt(Expr::Record(record), vec![M::id().into()])
        .eval(&[item.into()])
        .unwrap()
}
