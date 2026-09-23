use toasty::schema::Load;
use toasty::stmt::{Expr, IntoExpr, List};
use toasty_core::stmt::{self, Type, Value};

#[test]
fn option_values_preserve_presence() {
    let none: Expr<Option<String>> = None::<String>.into_expr();
    let some: Expr<Option<String>> = "hello".into_expr();
    let nested: Expr<Option<Option<String>>> = Some(None::<String>).into_expr();
    let eval = |expr| stmt::Expr::from(expr).eval_const().unwrap();
    assert_eq!(eval(none), Value::Option(None));
    assert_eq!(
        eval(some),
        Value::Option(Some(Box::new(Value::from("hello"))))
    );
    assert_eq!(
        stmt::Expr::from(nested).eval_const().unwrap(),
        Value::Option(Some(Box::new(Value::Option(None))))
    );
    assert_eq!(
        <Box<Option<String>> as Load>::app_ty(),
        Type::Option(Box::new(Type::String))
    );
    assert_eq!(
        <List<Option<String>> as Load>::app_ty(),
        Type::list(Type::Option(Box::new(Type::String)))
    );
}

#[test]
fn option_evaluation_matches_rust() {
    for lhs in [None, Some(-1_i64), Some(0)] {
        for rhs in [None, Some(-1_i64), Some(0)] {
            let expr: Expr<Option<i64>> = lhs.into_expr();
            for (predicate, expected) in [
                (expr.clone().eq(rhs), lhs == rhs),
                (expr.clone().ne(rhs), lhs != rhs),
                (expr.clone().lt(rhs), lhs < rhs),
                (expr.clone().le(rhs), lhs <= rhs),
                (expr.clone().gt(rhs), lhs > rhs),
                (expr.clone().ge(rhs), lhs >= rhs),
                (expr.clone().is_none(), lhs.is_none()),
                (expr.clone().is_some(), lhs.is_some()),
                (toasty::stmt::in_list(expr, [rhs]), [rhs].contains(&lhs)),
            ] {
                assert_eq!(
                    stmt::Expr::from(predicate).eval_const().unwrap(),
                    Value::Bool(expected)
                );
            }
        }
    }
}

#[test]
fn database_null_is_not_application_none() {
    let null_eq = stmt::Expr::eq(Value::Null, Value::Null);
    assert_eq!(null_eq.eval_const().unwrap(), Value::Null);
    assert_eq!(stmt::Expr::not(null_eq).eval_const().unwrap(), Value::Null);
    let none: Expr<Option<i64>> = None::<i64>.into_expr();
    assert_eq!(
        stmt::Expr::from(none.eq(None::<i64>)).eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn option_nested_equality_keeps_each_presence_level() {
    for lhs in [None, Some(None), Some(Some(1_i64))] {
        for rhs in [None, Some(None), Some(Some(1_i64))] {
            let expr: Expr<Option<Option<i64>>> = lhs.into_expr();
            assert_eq!(
                stmt::Expr::from(expr.eq(rhs)).eval_const().unwrap(),
                Value::Bool(lhs == rhs)
            );
        }
    }
}
