use super::*;
use crate::engine::test_util::test_schema;
use stmt::{Expr, Query, Value, Values};

#[test]
fn evaluable_expressions_build_and_run() {
    let schema = test_schema();
    for (expr, expected) in [
        (Expr::Static(Value::I64(42)), Value::I64(42)),
        (Expr::in_list(42, Expr::list([42])), Value::Bool(true)),
        (Expr::any(Expr::list([false, true])), Value::Bool(true)),
        (
            Expr::exists(Query::values(Values::new(vec![42.into()]))),
            Value::Bool(true),
        ),
    ] {
        let func = Func::try_from_stmt(&expr, vec![]).unwrap();
        assert_eq!(func.eval_const(&schema), expected);
        let func = Func::from_stmt(expr, vec![]);
        assert_eq!(func.eval_const(&schema), expected);
    }
}

#[test]
fn unsupported_expressions_return_none() {
    for expr in [
        Expr::between(2, 1, 3),
        Expr::match_expr(1, vec![], Expr::count_star()),
    ] {
        assert!(Func::try_from_stmt(&expr, vec![]).is_none());
    }
}
