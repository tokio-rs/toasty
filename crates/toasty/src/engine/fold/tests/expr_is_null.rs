use crate::engine::fold::{expr_is_null::fold_expr_is_null, fold_stmt};
use toasty_core::stmt::{Expr, ExprArg, ExprCast, ExprIsNull, Type, Value};

#[test]
fn cast_is_stripped_from_is_null() {
    // `is_null(cast(arg(0), String))` → `is_null(arg(0))`
    // Nullity is type-independent, so the cast is stripped.
    let mut expr = ExprIsNull {
        expr: Box::new(Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::String,
        })),
    };
    let result = fold_expr_is_null(&mut expr);

    assert!(result.is_none());
    assert!(matches!(*expr.expr, Expr::Arg(ExprArg { position: 0, .. })));
}

#[test]
fn non_cast_expr_not_simplified() {
    // `is_null(arg(0))`, non-cast, not simplified
    let mut expr = ExprIsNull {
        expr: Box::new(Expr::arg(0)),
    };
    let result = fold_expr_is_null(&mut expr);

    assert!(result.is_none());
    assert!(matches!(*expr.expr, Expr::Arg(ExprArg { position: 0, .. })));
}

#[test]
fn null_is_null_becomes_true() {
    // `null is null` → `true`
    let mut expr = ExprIsNull {
        expr: Box::new(Expr::null()),
    };
    let result = fold_expr_is_null(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn non_null_const_is_null_becomes_false() {
    // `5 is null` → `false`
    let mut expr = ExprIsNull {
        expr: Box::new(Expr::from(5i64)),
    };
    let result = fold_expr_is_null(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn null_is_not_null_becomes_false() {
    // `not(is_null(null))` → `not(true)` → `false`
    let mut expr = Expr::is_not_null(Expr::null());
    fold_stmt(&mut expr);

    assert!(matches!(expr, Expr::Value(Value::Bool(false))));
}

#[test]
fn non_null_const_is_not_null_becomes_true() {
    // `not(is_null(5))` → `not(false)` → `true`
    let mut expr = Expr::is_not_null(Expr::from(5i64));
    fold_stmt(&mut expr);

    assert!(matches!(expr, Expr::Value(Value::Bool(true))));
}
