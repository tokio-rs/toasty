use super::Query;

use crate::{engine::eval::Func, Executor, ExecutorExt, Load, Result};

use toasty_core::stmt::{self, visit_mut, Expr, ExprRecord, OrderBy, Projection, Value, VisitMut};

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
        // Extract the limit from the query to determine page size
        let page_size = match &self.query.untyped.limit {
            Some(stmt::Limit {
                limit: stmt::Expr::Value(stmt::Value::I64(n)),
                ..
            }) => *n as usize,
            _ => {
                eprintln!("pagination requires a limit clause");
                let values: Vec<Value> = executor
                    .exec(self.query.clone().into())
                    .await?
                    .collect()
                    .await?;
                let items: Vec<M::Output> =
                    values.into_iter().map(M::load).collect::<Result<_>>()?;
                return Ok(crate::Page::new(
                    items,
                    Query::from_untyped(self.query.untyped),
                    None,
                    None,
                ));
            }
        };
        eprintln!("Pagination {:?}", page_size);
        // Query for one more item than requested to detect if there's a next page
        let mut query_with_extra = self.query.clone();
        if let Some(stmt::Limit { limit, .. }) = &mut query_with_extra.untyped.limit {
            *limit = stmt::Value::from((page_size + 1) as i64).into();
        }

        let Some(order_by) = query_with_extra.untyped.order_by.as_mut() else {
            panic!("pagination requires order by clause");
        };
        if self.reverse {
            // Check if the driver supports reverse pagination
            if !executor.capability().sql {
                todo!("Determine if nosql can support this");
            }
            order_by.reverse();
        }

        let mut stream = executor.exec(query_with_extra.into()).await?;

        // Extract driver-provided cursor before consuming the stream
        let driver_cursor = stream.take_cursor();
        eprintln!("driver_cursor = {:?}", driver_cursor);
        let mut items: Vec<_> = stream.collect().await?;
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

        // For next cursor: prefer driver-provided cursor (e.g., DynamoDB's last_evaluated_key)
        // over extracting from the last item. SQL drivers won't provide a cursor.
        let next_cursor = driver_cursor.map(|cursor| cursor.into()).or_else(|| {
            // TODO: DDB cursor will be none on terminate. Need a test here?
            let has_next = (items.len() > page_size) || self.reverse;
            if executor.capability().sql && has_next {
                items.last().and_then(|last_item| {
                    extract_cursor(order_by, last_item).map(|cursor| cursor.into())
                })
            } else {
                None
            }
        });
        eprintln!("Next cursor: {:?}", next_cursor);
        // Load the raw values into model instances
        let loaded_items: Vec<M::Output> = items.into_iter().map(M::load).collect::<Result<_>>()?;

        Ok(crate::Page::new(
            loaded_items,
            Query::from_untyped(self.query.untyped),
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
