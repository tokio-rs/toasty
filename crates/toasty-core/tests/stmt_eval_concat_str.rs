use toasty_core::stmt::{ConstInput, Expr, Value};

// ---------------------------------------------------------------------------
// Basic concatenation
// ---------------------------------------------------------------------------

#[test]
fn concat_two_strings() {
    assert_eq!(
        Expr::concat_str(("hello", " world")).eval_const().unwrap(),
        Value::String("hello world".to_owned())
    );
}

#[test]
fn concat_three_strings() {
    assert_eq!(
        Expr::concat_str(("a", "b", "c")).eval_const().unwrap(),
        Value::String("abc".to_owned())
    );
}

#[test]
fn concat_single_string() {
    assert_eq!(
        Expr::concat_str(("only",)).eval_const().unwrap(),
        Value::String("only".to_owned())
    );
}

#[test]
fn concat_empty_strings() {
    assert_eq!(
        Expr::concat_str(("", "")).eval_const().unwrap(),
        Value::String(String::new())
    );
}

#[test]
fn concat_one_empty_one_nonempty() {
    assert_eq!(
        Expr::concat_str(("", "hello")).eval_const().unwrap(),
        Value::String("hello".to_owned())
    );
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::concat_str(("foo", "bar"));
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}

// ---------------------------------------------------------------------------
// Error: non-string value inside ConcatStr
// ---------------------------------------------------------------------------

#[test]
fn concat_str_with_bool_is_error() {
    use toasty_core::stmt::ExprConcatStr;
    // Manually build a ConcatStr containing a non-string expr.
    let expr = Expr::ConcatStr(ExprConcatStr {
        exprs: vec![Expr::from(true)],
    });
    assert!(expr.eval_const().is_err());
}

#[test]
fn concat_str_with_i64_is_error() {
    use toasty_core::stmt::ExprConcatStr;
    let expr = Expr::ConcatStr(ExprConcatStr {
        exprs: vec![Expr::from(42i64)],
    });
    assert!(expr.eval_const().is_err());
}

#[test]
fn concat_str_with_null_is_error() {
    use toasty_core::stmt::ExprConcatStr;
    let expr = Expr::ConcatStr(ExprConcatStr {
        exprs: vec![Expr::from(Value::Null)],
    });
    assert!(expr.eval_const().is_err());
}
