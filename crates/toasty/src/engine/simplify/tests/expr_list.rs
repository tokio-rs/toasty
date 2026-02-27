use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprList, Value};

#[test]
fn all_const_values_become_value_list() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `list([1, 2, 3]) → [1, 2, 3]`
    let mut expr = ExprList {
        items: vec![
            Expr::Value(Value::from(1i64)),
            Expr::Value(Value::from(2i64)),
            Expr::Value(Value::from(3i64)),
        ],
    };
    let result = simplify.simplify_expr_list(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::List(list)) = result.unwrap() else {
        panic!("expected result to be a `Value::List`");
    };
    assert_eq!(list.len(), 3);
    assert_eq!(list[0], Value::from(1i64));
    assert_eq!(list[1], Value::from(2i64));
    assert_eq!(list[2], Value::from(3i64));
}

#[test]
fn mixed_types_in_list() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `list(["hello", 42, true]) → ["hello", 42, true]`
    let mut expr = ExprList {
        items: vec![
            Expr::Value(Value::from("hello")),
            Expr::Value(Value::from(42i64)),
            Expr::Value(Value::from(true)),
        ],
    };
    let result = simplify.simplify_expr_list(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::List(list)) = result.unwrap() else {
        panic!("expected result to be a `Value::List`");
    };
    assert_eq!(list.len(), 3);
    assert_eq!(list[0], Value::from("hello"));
    assert_eq!(list[1], Value::from(42i64));
    assert_eq!(list[2], Value::from(true));
}

#[test]
fn empty_list_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `list([]) → []`
    let mut expr = ExprList { items: vec![] };
    let result = simplify.simplify_expr_list(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::List(list)) = result.unwrap() else {
        panic!("expected result to be a `Value::List`");
    };
    assert!(list.is_empty());
}

#[test]
fn non_const_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `list([1, arg(0)])`, non-constant, not simplified
    let mut expr = ExprList {
        items: vec![Expr::Value(Value::from(1i64)), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_list(&mut expr);

    assert!(result.is_none());
}

#[test]
fn list_with_error_item_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `list([1, error("boom")])` — error is not a Value, so not folded
    let mut expr = ExprList {
        items: vec![Expr::Value(Value::from(1i64)), Expr::error("boom")],
    };
    let result = simplify.simplify_expr_list(&mut expr);

    assert!(result.is_none());
}
