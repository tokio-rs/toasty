use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::driver::Capability;
use toasty_core::stmt::{BinaryOp, Expr, ExprIsSuperset, Value};

fn is_superset(rhs: Expr) -> ExprIsSuperset {
    ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(rhs),
    }
}

#[test]
fn rewrites_value_list_into_and_of_any_op_when_not_native() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema, &Capability::DYNAMODB);

    let mut expr = is_superset(Expr::Value(Value::List(vec![Value::I64(1), Value::I64(2)])));
    let result = simplify
        .simplify_expr_is_superset(&mut expr)
        .expect("expected rewrite");

    let Expr::And(and) = result else {
        panic!("expected And, got {result:#?}");
    };
    assert_eq!(and.operands.len(), 2);
    for operand in &and.operands {
        let Expr::AnyOp(any) = operand else {
            panic!("expected AnyOp, got {operand:#?}");
        };
        assert!(matches!(any.op, BinaryOp::Eq));
    }
}

#[test]
fn leaves_node_alone_when_driver_is_native() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema, &Capability::SQLITE);

    let mut expr = is_superset(Expr::Value(Value::List(vec![Value::I64(1)])));
    let result = simplify.simplify_expr_is_superset(&mut expr);
    assert!(result.is_none());
}

#[test]
fn leaves_node_alone_when_rhs_is_not_a_value_list() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema, &Capability::DYNAMODB);

    // A non-list rhs cannot be expanded; the capability check in `verify`
    // rejects this shape before exec, but the simplify pass must not panic.
    let mut expr = is_superset(Expr::arg(1));
    let result = simplify.simplify_expr_is_superset(&mut expr);
    assert!(result.is_none());
}
