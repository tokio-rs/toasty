use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{self, Direction, Limit, OrderBy, OrderByExpr, Query, Values};

#[test]
fn empty_values_query_is_empty() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `stmt_query_is_empty(values([])) → true`
    let query = Query::values(Values::default());
    assert!(simplify.stmt_query_is_empty(&query));
}

#[test]
fn non_empty_values_query_not_empty() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `stmt_query_is_empty(values([1])) → false`
    let mut values = Values::default();
    values.rows.push(stmt::Expr::Value(stmt::Value::from(1i64)));
    let query = Query::values(values);
    assert!(!simplify.stmt_query_is_empty(&query));
}

#[test]
fn simplify_clears_order_by_and_limit() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `simplify_stmt_query_when_empty(empty_query)` clears `order_by` and
    // `limit`
    let mut query = Query::values(Values::default());
    query.order_by = Some(OrderBy {
        exprs: vec![OrderByExpr {
            expr: stmt::Expr::Value(stmt::Value::from(1i64)),
            order: Some(Direction::Asc),
        }],
    });
    query.limit = Some(Limit {
        limit: stmt::Expr::Value(stmt::Value::from(10i64)),
        offset: None,
    });

    simplify.simplify_stmt_query_when_empty(&mut query);

    assert!(query.order_by.is_none());
    assert!(query.limit.is_none());
}

#[test]
fn non_empty_query_keeps_order_by_and_limit() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `simplify_stmt_query_when_empty(non_empty_query)` keeps `order_by`
    // and `limit`
    let mut values = Values::default();
    values.rows.push(stmt::Expr::Value(stmt::Value::from(1i64)));
    let mut query = Query::values(values);
    query.order_by = Some(OrderBy {
        exprs: vec![OrderByExpr {
            expr: stmt::Expr::Value(stmt::Value::from(1i64)),
            order: Some(Direction::Desc),
        }],
    });
    query.limit = Some(Limit {
        limit: stmt::Expr::Value(stmt::Value::from(10i64)),
        offset: None,
    });

    simplify.simplify_stmt_query_when_empty(&mut query);

    assert!(query.order_by.is_some());
    assert!(query.limit.is_some());
}
