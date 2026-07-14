use crate::engine::fold;
use proptest::prelude::*;
use toasty_core::stmt::{BinaryOp, Expr, ExprAny, ExprBinaryOp, ExprInList, ExprList, Value};

fn arb_i64_value() -> impl Strategy<Value = Expr> {
    (-100i64..=100).prop_map(|n| Expr::Value(Value::I64(n)))
}

fn arb_i64_raw() -> impl Strategy<Value = i64> {
    -100i64..=100
}

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

/// Generates a pure constant boolean expression. Mirrors the generator in
/// `simplify::tests::prop_const` so fold idempotence is exercised on the
/// same shapes that the simplifier already handles.
fn arb_bool_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        Just(Expr::from(true)),
        Just(Expr::from(false)),
        (arb_i64_value(), arb_binary_op(), arb_i64_value()).prop_map(|(lhs, op, rhs)| {
            Expr::BinaryOp(ExprBinaryOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            })
        }),
        arb_i64_value().prop_map(Expr::is_null),
        Just(Expr::is_null(Expr::null())),
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

    leaf.prop_recursive(4, 64, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(Expr::not),
            prop::collection::vec(inner.clone(), 2..=4).prop_map(Expr::and_from_vec),
            prop::collection::vec(inner.clone(), 2..=4).prop_map(Expr::or_from_vec),
            prop::collection::vec(inner.clone(), 1..=4).prop_map(|items| {
                Expr::Any(ExprAny {
                    expr: Box::new(Expr::List(ExprList { items })),
                })
            }),
            (inner.clone(), any::<bool>(), any::<bool>()).prop_map(|(expr, b, use_eq)| {
                let op = if use_eq { BinaryOp::Eq } else { BinaryOp::Ne };
                Expr::binary_op(expr, op, Expr::from(b))
            }),
        ]
    })
}

proptest! {
    /// `fold(fold(x)) == fold(x)`. Required for fold to be safely composed
    /// from `lower`, `simplify`, and `exec_statement` without surprises.
    #[test]
    fn fold_is_idempotent(expr in arb_bool_expr()) {
        let once = {
            let mut e = expr.clone();
            fold::fold_stmt(&mut e);
            e
        };
        let twice = {
            let mut e = once.clone();
            fold::fold_stmt(&mut e);
            e
        };
        prop_assert!(once.is_equivalent_to(&twice), "once = {once:?}, twice = {twice:?}");
    }
}
