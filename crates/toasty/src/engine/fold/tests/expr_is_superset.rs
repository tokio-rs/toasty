use crate::engine::fold::expr_is_superset::fold_expr_is_superset;
use toasty_core::stmt::{self, Expr, Value};

fn is_superset(lhs: Expr, rhs: Expr) -> stmt::ExprIsSuperset {
    stmt::ExprIsSuperset {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

#[test]
fn empty_value_list_rhs_becomes_true() {
    // `is_superset(arg(0), []) → true`
    let mut expr = is_superset(Expr::arg(0), Expr::Value(Value::List(vec![])));
    let result = fold_expr_is_superset(&mut expr);

    assert_eq!(result, Some(Expr::Value(Value::Bool(true))));
}

#[test]
fn empty_expr_list_rhs_becomes_true() {
    // `is_superset(arg(0), list([])) → true`
    let mut expr = is_superset(Expr::arg(0), Expr::List(stmt::ExprList { items: vec![] }));
    let result = fold_expr_is_superset(&mut expr);

    assert_eq!(result, Some(Expr::Value(Value::Bool(true))));
}

#[test]
fn non_empty_rhs_left_alone() {
    let mut expr = is_superset(
        Expr::arg(0),
        Expr::Value(Value::List(vec![Value::from("admin".to_string())])),
    );
    let result = fold_expr_is_superset(&mut expr);

    assert!(result.is_none());
}
