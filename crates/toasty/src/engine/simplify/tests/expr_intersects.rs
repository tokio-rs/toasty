use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::driver::Capability;
use toasty_core::stmt::{BinaryOp, Expr, ExprIntersects, Value};

fn intersects(rhs: Expr) -> ExprIntersects {
    ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(rhs),
    }
}

#[test]
fn rewrites_value_list_into_or_of_any_op_when_not_native() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema, &Capability::DYNAMODB);

    let mut expr = intersects(Expr::Value(Value::List(vec![Value::I64(1), Value::I64(2)])));
    let result = simplify
        .simplify_expr_intersects(&mut expr)
        .expect("expected rewrite");

    let Expr::Or(or) = result else {
        panic!("expected Or, got {result:#?}");
    };
    assert_eq!(or.operands.len(), 2);
    for operand in &or.operands {
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

    let mut expr = intersects(Expr::Value(Value::List(vec![Value::I64(1)])));
    let result = simplify.simplify_expr_intersects(&mut expr);
    assert!(result.is_none());
}
