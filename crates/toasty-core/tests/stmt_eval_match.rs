use toasty_core::stmt::{ConstInput, Expr, MatchArm, Value};

fn two_arm_match(subject: impl Into<Expr>) -> Expr {
    Expr::match_expr(
        subject,
        vec![
            MatchArm {
                pattern: Value::from(0i64),
                expr: Expr::from("first"),
            },
            MatchArm {
                pattern: Value::from(1i64),
                expr: Expr::from("second"),
            },
        ],
        Expr::null(),
    )
}

#[test]
fn match_first_arm() {
    assert_eq!(
        two_arm_match(Expr::from(0i64)).eval_const().unwrap(),
        Value::from("first")
    );
}

#[test]
fn match_second_arm() {
    assert_eq!(
        two_arm_match(Expr::from(1i64)).eval_const().unwrap(),
        Value::from("second")
    );
}

#[test]
fn match_no_arm_falls_through_to_else() {
    assert_eq!(
        two_arm_match(Expr::from(99i64)).eval_const().unwrap(),
        Value::Null
    );
}

#[test]
fn match_with_arg_subject() {
    let expr = two_arm_match(Expr::arg(0usize));
    let input = vec![Value::from(1i64)];
    assert_eq!(expr.eval(&input).unwrap(), Value::from("second"));
}

#[test]
fn match_arm_expr_evaluated() {
    let expr = Expr::match_expr(
        Expr::from(0i64),
        vec![MatchArm {
            pattern: Value::from(0i64),
            expr: Expr::record([Expr::from(42i64), Expr::from("hello")]),
        }],
        Expr::null(),
    );
    let result = expr.eval_const().unwrap();
    match result {
        Value::Record(record) => {
            assert_eq!(record[0], Value::I64(42));
            assert_eq!(record[1], Value::from("hello"));
        }
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn eval_with_input_agrees() {
    let expr = two_arm_match(Expr::from(0i64));
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
