use toasty_core::stmt::{
    Expr, Limit, Offset, OrderBy, OrderByExpr, Query, Statement, Value, Values,
};

fn values_query(items: Vec<i64>) -> Query {
    Query::values(Values::new(
        items
            .into_iter()
            .map(|n| Expr::from(Value::I64(n)))
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// ORDER BY — not evaluable client-side
// ---------------------------------------------------------------------------

#[test]
fn query_with_order_by_is_error() {
    let mut query = values_query(vec![1, 2, 3]);
    query.order_by = Some(OrderBy::from(OrderByExpr {
        expr: Expr::from(0i64),
        order: None,
    }));
    assert!(Statement::Query(query).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// LIMIT — slices the result list
// ---------------------------------------------------------------------------

#[test]
fn limit_returns_first_n_items() {
    let mut query = values_query(vec![1, 2, 3, 4, 5]);
    query.limit = Some(Limit {
        limit: Expr::from(3i64),
        offset: None,
    });
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(
        result,
        Value::List(vec![Value::I64(1), Value::I64(2), Value::I64(3)])
    );
}

#[test]
fn limit_larger_than_list_returns_all() {
    let mut query = values_query(vec![1, 2]);
    query.limit = Some(Limit {
        limit: Expr::from(10i64),
        offset: None,
    });
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(result, Value::List(vec![Value::I64(1), Value::I64(2)]));
}

#[test]
fn limit_zero_returns_empty_list() {
    let mut query = values_query(vec![1, 2, 3]);
    query.limit = Some(Limit {
        limit: Expr::from(0i64),
        offset: None,
    });
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(result, Value::List(vec![]));
}

#[test]
fn limit_with_count_offset_skips_then_takes() {
    let mut query = values_query(vec![1, 2, 3, 4, 5]);
    query.limit = Some(Limit {
        limit: Expr::from(2i64),
        offset: Some(Offset::Count(Expr::from(2i64))),
    });
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(result, Value::List(vec![Value::I64(3), Value::I64(4)]));
}

#[test]
fn limit_with_count_offset_larger_than_list_returns_empty() {
    let mut query = values_query(vec![1, 2]);
    query.limit = Some(Limit {
        limit: Expr::from(5i64),
        offset: Some(Offset::Count(Expr::from(10i64))),
    });
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(result, Value::List(vec![]));
}

#[test]
fn limit_with_keyset_offset_is_error() {
    let mut query = values_query(vec![1, 2, 3]);
    query.limit = Some(Limit {
        limit: Expr::from(2i64),
        offset: Some(Offset::After(Expr::from(1i64))),
    });
    assert!(Statement::Query(query).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// single — unwraps a one-element list
// ---------------------------------------------------------------------------

#[test]
fn single_with_one_row_returns_value() {
    let mut query = values_query(vec![42]);
    query.single = true;
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(result, Value::I64(42));
}

#[test]
fn single_with_zero_rows_is_error() {
    let mut query = values_query(vec![]);
    query.single = true;
    assert!(Statement::Query(query).eval_const().is_err());
}

#[test]
fn single_with_two_rows_is_error() {
    let mut query = values_query(vec![1, 2]);
    query.single = true;
    assert!(Statement::Query(query).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// Baseline — a plain Values query evaluates fine
// ---------------------------------------------------------------------------

#[test]
fn plain_values_query_evals_ok() {
    let query = Query::values(Values::new(vec![
        Expr::from(Value::I64(1)),
        Expr::from(Value::I64(2)),
    ]));
    let result = Statement::Query(query).eval_const().unwrap();
    assert_eq!(result, Value::List(vec![Value::I64(1), Value::I64(2)]));
}
