use super::test_schema;
use crate::engine::simplify::Simplify;
use proptest::prelude::*;
use toasty_core::stmt::{
    BinaryOp, Expr, ExprAny, ExprBinaryOp, ExprInList, ExprList, Value, VisitMut,
};

/// Generates a leaf integer expression in the range -100..=100.
fn arb_i64_value() -> impl Strategy<Value = Expr> {
    (-100i64..=100).prop_map(|n| Expr::Value(Value::I64(n)))
}

/// Generates a raw i64 in the range -100..=100.
fn arb_i64_raw() -> impl Strategy<Value = i64> {
    -100i64..=100
}

/// Generates one of the six comparison operators.
fn arb_binary_op() -> impl Strategy<Value = BinaryOp> {
    prop_oneof![
        Just(BinaryOp::Eq),
        Just(BinaryOp::Ne),
        Just(BinaryOp::Lt),
        Just(BinaryOp::Le),
        Just(BinaryOp::Gt),
        Just(BinaryOp::Ge),
    ]
}

/// Generates a pure constant boolean expression that can be evaluated
/// without a database or schema via `eval_const()`.
///
/// Leaf cases are always evaluable and never produce null in a boolean
/// position. Recursive cases combine boolean sub-expressions.
fn arb_bool_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        // Literal true/false
        Just(Expr::from(true)),
        Just(Expr::from(false)),
        // BinaryOp(i64, op, i64) → bool
        (arb_i64_value(), arb_binary_op(), arb_i64_value()).prop_map(|(lhs, op, rhs)| {
            Expr::BinaryOp(ExprBinaryOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            })
        }),
        // IsNull(i64) → always false, IsNull(null) → always true
        arb_i64_value().prop_map(Expr::is_null),
        Just(Expr::is_null(Expr::null())),
        // InList(i64, [i64; 0..=3]) → bool, exercises empty/single/multi list paths
        (arb_i64_value(), prop::collection::vec(arb_i64_raw(), 0..=3)).prop_map(
            |(expr, values)| {
                let list = Expr::Value(Value::List(values.into_iter().map(Value::I64).collect()));
                Expr::InList(ExprInList {
                    expr: Box::new(expr),
                    list: Box::new(list),
                })
            }
        ),
    ];

    leaf.prop_recursive(
        4,  // max recursion depth
        64, // max total nodes
        4,  // max items per collection
        |inner| {
            prop_oneof![
                // Not(bool_expr) — double negation, De Morgan's, constant folding
                inner.clone().prop_map(Expr::not),
                // And([bool_expr; 2..=4]) — flattening, identity, contradiction
                prop::collection::vec(inner.clone(), 2..=4).prop_map(Expr::and_from_vec),
                // Or([bool_expr; 2..=4]) — flattening, OR-to-IN conversion
                prop::collection::vec(inner.clone(), 2..=4).prop_map(Expr::or_from_vec),
                // Any(List([bool_expr; 1..=4])) — Any+List simplifiers
                prop::collection::vec(inner.clone(), 1..=4).prop_map(|items| {
                    Expr::Any(ExprAny {
                        expr: Box::new(Expr::List(ExprList { items })),
                    })
                }),
                // BinaryOp(bool_expr, Eq/Ne, bool_literal) → bool_expr or not(bool_expr)
                (inner.clone(), any::<bool>(), any::<bool>()).prop_map(|(expr, b, use_eq)| {
                    let op = if use_eq { BinaryOp::Eq } else { BinaryOp::Ne };
                    Expr::binary_op(expr, op, Expr::from(b))
                }),
            ]
        },
    )
}

proptest! {
    #[test]
    fn simplify_preserves_eval(expr in arb_bool_expr()) {
        let schema = test_schema();
        let oracle = expr.eval_const().expect("generated expr must be evaluable");

        let mut expr = expr;
        Simplify::new(&schema).visit_expr_mut(&mut expr);

        let result = expr.eval_const().expect("simplified expr must be evaluable");
        prop_assert_eq!(oracle, result);
    }
}
