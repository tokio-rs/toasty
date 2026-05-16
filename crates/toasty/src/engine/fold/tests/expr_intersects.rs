use crate::engine::fold::expr_intersects::fold_expr_intersects;
use toasty_core::stmt::{self, Expr, Value};

fn intersects(lhs: Expr, rhs: Expr) -> stmt::ExprIntersects {
    stmt::ExprIntersects {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

#[test]
fn empty_value_list_rhs_becomes_false() {
    // `intersects(arg(0), []) → false`
    let mut expr = intersects(Expr::arg(0), Expr::Value(Value::List(vec![])));
    let result = fold_expr_intersects(&mut expr);

    assert_eq!(result, Some(Expr::Value(Value::Bool(false))));
}

#[test]
fn empty_expr_list_rhs_becomes_false() {
    // `intersects(arg(0), list([])) → false`
    let mut expr = intersects(Expr::arg(0), Expr::List(stmt::ExprList { items: vec![] }));
    let result = fold_expr_intersects(&mut expr);

    assert_eq!(result, Some(Expr::Value(Value::Bool(false))));
}

#[test]
fn non_empty_rhs_left_alone() {
    let mut expr = intersects(
        Expr::arg(0),
        Expr::Value(Value::List(vec![Value::from("admin".to_string())])),
    );
    let result = fold_expr_intersects(&mut expr);

    assert!(result.is_none());
}
