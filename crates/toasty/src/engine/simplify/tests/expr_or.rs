use super::{test_schema, test_schema_with};
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprOr};

#[test]
fn idempotent_two_identical() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `or(a, a) → a`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_three_identical() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `or(a, a, a) → a`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(0), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_with_different() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `or(a, b, a) → or(a, b)`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn absorption_or_and() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `or(a, and(a, b))` → `a`
    let mut expr = ExprOr {
        operands: vec![
            Expr::arg(0),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn absorption_with_multiple_operands() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `or(a, b, and(a, c))` → `or(a, b)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::arg(0),
            Expr::arg(1),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn absorption_two_or_three_and() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `or(a, b, and(a, c, d))` → `or(a, b)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::arg(0),
            Expr::arg(1),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn factoring_basic() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a and b) or (a and c)` → `a and (b or c)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Result should be `a and (b or c)`
    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And");
    };
    assert_eq!(and_expr.operands.len(), 2);
    assert_eq!(and_expr.operands[0], Expr::arg(0));

    let Expr::Or(or_expr) = &and_expr.operands[1] else {
        panic!("expected Or");
    };
    assert_eq!(or_expr.operands.len(), 2);
    assert_eq!(or_expr.operands[0], Expr::arg(1));
    assert_eq!(or_expr.operands[1], Expr::arg(2));
}

#[test]
fn factoring_multiple_common() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a and b and c) or (a and b and d)` → `a and b and (c or d)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(2)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Result should be `a and b and (c or d)`
    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And");
    };
    assert_eq!(and_expr.operands.len(), 3);
    assert_eq!(and_expr.operands[0], Expr::arg(0));
    assert_eq!(and_expr.operands[1], Expr::arg(1));

    let Expr::Or(or_expr) = &and_expr.operands[2] else {
        panic!("expected Or");
    };
    assert_eq!(or_expr.operands.len(), 2);
    assert_eq!(or_expr.operands[0], Expr::arg(2));
    assert_eq!(or_expr.operands[1], Expr::arg(3));
}

#[test]
fn factoring_three_ands() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a and b) or (a and c) or (a and d)` → `a and (b or c or d)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Result should be `a and (b or c or d)`
    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And");
    };
    assert_eq!(and_expr.operands.len(), 2);
    assert_eq!(and_expr.operands[0], Expr::arg(0));

    let Expr::Or(or_expr) = &and_expr.operands[1] else {
        panic!("expected Or");
    };
    assert_eq!(or_expr.operands.len(), 3);
    assert_eq!(or_expr.operands[0], Expr::arg(1));
    assert_eq!(or_expr.operands[1], Expr::arg(2));
    assert_eq!(or_expr.operands[2], Expr::arg(3));
}

#[test]
fn factoring_no_common() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a and b) or (c and d)` → no change (no common factor)
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(2), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
}

#[test]
fn complement_basic() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a or not(a)` → `true` (where a is a non-nullable comparison)
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprOr {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn complement_with_other_operands() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a or b or not(a)` → `true`
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprOr {
        operands: vec![
            a.clone(),
            Expr::arg(2),
            Expr::Not(ExprNot { expr: Box::new(a) }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn complement_nullable_not_simplified() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a or not(a)` where `a` is an arg (nullable) → no change
    let a = Expr::arg(0);
    let mut expr = ExprOr {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
}

#[test]
fn complement_multiple_repetitions() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a or a or not(a) or not(a)` → `true`
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprOr {
        operands: vec![
            a.clone(),
            a.clone(),
            Expr::Not(ExprNot {
                expr: Box::new(a.clone()),
            }),
            Expr::Not(ExprNot { expr: Box::new(a) }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

// OR-to-IN conversion tests

#[test]
fn or_to_in_basic() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1 or a = 2 or a = 3` → `a in (1, 2, 3)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(3i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    let Some(Expr::InList(in_list)) = result else {
        panic!("expected InList");
    };
    assert_eq!(*in_list.expr, Expr::arg(0));
}

#[test]
fn or_to_in_two_values() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1 or a = 2` → `a in (1, 2)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(matches!(result, Some(Expr::InList(_))));
}

#[test]
fn or_to_in_single_value_not_converted() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1` (single equality, not converted)
    let mut expr = ExprOr {
        operands: vec![Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64)))],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Single operand gets unwrapped, but not to IN
    assert!(result.is_some());
    assert!(matches!(result, Some(Expr::BinaryOp(_))));
}

#[test]
fn or_to_in_different_lhs_not_converted() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1 or b = 2` (different LHS, not converted to single IN)
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(2i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Not converted, stays as OR
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn or_to_in_mixed_keeps_other_operands() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1 or a = 2 or b = 3` → `a in (1, 2) or b = 3`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(3i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Stays as OR but with transformed operands
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);

    // Should have one InList and one BinaryOp
    let has_in_list = expr.operands.iter().any(|e| matches!(e, Expr::InList(_)));
    let has_binary_op = expr.operands.iter().any(|e| matches!(e, Expr::BinaryOp(_)));
    assert!(has_in_list);
    assert!(has_binary_op);
}

#[test]
fn or_to_in_multiple_groups() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1 or a = 2 or b = 3 or b = 4` → `a in (1, 2) or b in (3, 4)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(3i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(4i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Stays as OR with two InList operands
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);

    // Both should be InList
    assert!(expr.operands.iter().all(|e| matches!(e, Expr::InList(_))));
}

#[test]
fn or_to_in_with_non_equality_preserved() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = 1 or a = 2 or c` → `a in (1, 2) or c`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::arg(2), // non-equality operand
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Stays as OR
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);

    // One InList, one arg
    let has_in_list = expr.operands.iter().any(|e| matches!(e, Expr::InList(_)));
    let has_arg = expr.operands.iter().any(|e| matches!(e, Expr::Arg(_)));
    assert!(has_in_list);
    assert!(has_arg);
}

#[test]
fn or_to_in_non_const_rhs_not_converted() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `a = b or a = c` (non-constant RHS, not converted)
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::arg(1)),
            Expr::eq(Expr::arg(0), Expr::arg(2)),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Not converted, stays as OR
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert!(expr.operands.iter().all(|e| matches!(e, Expr::BinaryOp(_))));
}

// ---------------------------------------------------------------------------
// Determinism-aware simplification (issue #236).
//
// See the matching block in `expr_and.rs`.  `is_equivalent_to` gates the
// idempotent, absorption, complement, factoring, and OR-to-IN rewrites on
// `is_stable`, so non-deterministic sub-expressions survive intact.
// ---------------------------------------------------------------------------

#[test]
fn idempotent_not_simplified_for_non_deterministic() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `LAST_INSERT_ID() OR LAST_INSERT_ID()` must retain both operands.
    let mut expr = ExprOr {
        operands: vec![Expr::last_insert_id(), Expr::last_insert_id()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn complement_not_simplified_for_non_deterministic() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `f() = 1 OR NOT (f() = 1)` — both draws are independent, so the
    // complement law must NOT fire.
    let a = Expr::eq(Expr::last_insert_id(), 1i64);
    let mut expr = ExprOr {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn factoring_not_simplified_for_non_deterministic() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(f() AND b) OR (f() AND c)` would factor to `f() AND (b OR c)` under
    // PartialEq, but the original evaluates `f()` twice while the factored
    // form evaluates it once — unsound for non-deterministic `f()`.
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::last_insert_id(), Expr::arg(0)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::last_insert_id(), Expr::arg(1)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // No factoring should have taken place — we still have two AND branches,
    // each with the original two operands.
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    for operand in &expr.operands {
        let Expr::And(and) = operand else {
            panic!("expected AND branch, got {operand:?}");
        };
        assert_eq!(and.operands.len(), 2);
    }
}

#[test]
fn or_to_in_list_not_simplified_for_non_deterministic() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `f() = 1 OR f() = 2` must NOT be rewritten to `f() IN (1, 2)`: the
    // former evaluates `f()` twice (two draws against 1 and 2), the latter
    // evaluates it once (one draw tested against a set).
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::last_insert_id(), 1i64),
            Expr::eq(Expr::last_insert_id(), 2i64),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    for operand in &expr.operands {
        assert!(matches!(operand, Expr::BinaryOp(_)));
    }
}

// Variant tautology tests

mod variant_tautology {
    use super::*;
    use crate as toasty;
    use crate::schema::Register;
    use toasty_core::schema::app::VariantId;

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum TwoVariant {
        #[column(variant = 1)]
        A,
        #[column(variant = 2)]
        B,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ThreeVariant {
        #[column(variant = 1)]
        X,
        #[column(variant = 2)]
        Y,
        #[column(variant = 3)]
        Z,
    }

    #[test]
    fn all_two_variants_becomes_true() {
        let schema = test_schema_with(&[TwoVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = TwoVariant::id();

        // `is_variant(x, 0) or is_variant(x, 1)` → `true`
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn all_three_variants_becomes_true() {
        let schema = test_schema_with(&[ThreeVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = ThreeVariant::id();

        // `is_variant(x, 0) or is_variant(x, 1) or is_variant(x, 2)` → `true`
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 2,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn subset_of_variants_not_simplified() {
        let schema = test_schema_with(&[ThreeVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = ThreeVariant::id();

        // `is_variant(x, 0) or is_variant(x, 1)` over 3-variant enum → no change
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn single_variant_of_two_not_simplified() {
        let schema = test_schema_with(&[TwoVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = TwoVariant::id();

        // `is_variant(x, 0) or other` — only 1 of 2 variants → no tautology
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::arg(1),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn all_variants_with_extra_operands_becomes_true() {
        let schema = test_schema_with(&[TwoVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = TwoVariant::id();

        // `is_variant(x, 0) or other_expr or is_variant(x, 1)` → `true`
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::arg(5),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn different_inner_exprs_not_simplified() {
        let schema = test_schema_with(&[TwoVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = TwoVariant::id();

        // `is_variant(x, 0) or is_variant(y, 1)` — different inner exprs → no change
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(1),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn duplicate_variants_not_simplified() {
        let schema = test_schema_with(&[ThreeVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = ThreeVariant::id();

        // `is_variant(x, 0) or is_variant(x, 0) or is_variant(x, 1)` — duplicates
        // cover only 2 of 3 variants after dedup → no tautology
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Idempotent fires first, removing the duplicate. Then 2 of 3 → no tautology.
        assert!(result.is_none());
    }

    #[test]
    fn reversed_order_becomes_true() {
        let schema = test_schema_with(&[TwoVariant::schema()]);
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        let model_id = TwoVariant::id();

        // `is_variant(x, 1) or is_variant(x, 0)` (reversed) → `true`
        let mut expr = ExprOr {
            operands: vec![
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 1,
                    },
                ),
                Expr::is_variant(
                    Expr::arg(0),
                    VariantId {
                        model: model_id,
                        index: 0,
                    },
                ),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn no_is_variant_operands_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

        // `arg(0) or arg(1)` — no IsVariant at all → no tautology
        let mut expr = ExprOr {
            operands: vec![Expr::arg(0), Expr::arg(1)],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
    }
}
