use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, Value};

#[test]
fn const_base_with_identity_map() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `map([1, 2, 3], x => x) → [1, 2, 3]`
    let mut expr = Expr::map(
        Expr::Value(Value::List(vec![
            Value::from(1i64),
            Value::from(2i64),
            Value::from(3i64),
        ])),
        Expr::arg(0),
    );
    let result = simplify.simplify_expr_map(&mut expr);

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
fn non_const_base_not_simplified() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `map(arg(0), x => x)`, non-constant base, not simplified
    let mut expr = Expr::map(Expr::arg(0), Expr::arg(0));
    let result = simplify.simplify_expr_map(&mut expr);

    assert!(result.is_none());
}
