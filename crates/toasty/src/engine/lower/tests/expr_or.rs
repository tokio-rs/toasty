//! Tests for `lower::expr_or::is_variant_tautology_or`.
//!
//! Migrated from `simplify::tests::expr_or::variant_tautology` when the
//! rewrite moved out of simplify and into the lowering walk.

use crate as toasty;
use crate::engine::lower::expr_or::is_variant_tautology_or;
use crate::engine::test_util::test_schema_with;
use crate::schema::Register;
use toasty_core::schema::app::VariantId;
use toasty_core::stmt::{Expr, ExprOr};

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
fn all_two_variants_is_tautology() {
    let schema = test_schema_with(&[TwoVariant::schema()]);
    let model_id = TwoVariant::id();

    let expr = ExprOr {
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

    assert!(is_variant_tautology_or(&schema, &expr));
}

#[test]
fn all_three_variants_is_tautology() {
    let schema = test_schema_with(&[ThreeVariant::schema()]);
    let model_id = ThreeVariant::id();

    let expr = ExprOr {
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

    assert!(is_variant_tautology_or(&schema, &expr));
}

#[test]
fn subset_of_variants_is_not_tautology() {
    let schema = test_schema_with(&[ThreeVariant::schema()]);
    let model_id = ThreeVariant::id();

    // 2 of 3 variants covered → not a tautology.
    let expr = ExprOr {
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

    assert!(!is_variant_tautology_or(&schema, &expr));
}

#[test]
fn single_variant_of_two_with_extra_operand_is_not_tautology() {
    let schema = test_schema_with(&[TwoVariant::schema()]);
    let model_id = TwoVariant::id();

    // Only 1 of 2 variants covered → not a tautology, regardless of the
    // additional non-IsVariant operand.
    let expr = ExprOr {
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

    assert!(!is_variant_tautology_or(&schema, &expr));
}

#[test]
fn all_variants_with_extra_operands_is_tautology() {
    let schema = test_schema_with(&[TwoVariant::schema()]);
    let model_id = TwoVariant::id();

    // The IsVariant operands alone cover all variants; an extra non-IsVariant
    // operand does not block the tautology.
    let expr = ExprOr {
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

    assert!(is_variant_tautology_or(&schema, &expr));
}

#[test]
fn different_inner_exprs_is_not_tautology() {
    let schema = test_schema_with(&[TwoVariant::schema()]);
    let model_id = TwoVariant::id();

    // `is_variant(x, 0) or is_variant(y, 1)` — different anchors mean the
    // pair could both be false at runtime.
    let expr = ExprOr {
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

    assert!(!is_variant_tautology_or(&schema, &expr));
}

#[test]
fn duplicate_variants_is_not_tautology() {
    let schema = test_schema_with(&[ThreeVariant::schema()]);
    let model_id = ThreeVariant::id();

    // Three IsVariants but only two distinct → 2 of 3 variants covered →
    // not a tautology.
    let expr = ExprOr {
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

    assert!(!is_variant_tautology_or(&schema, &expr));
}

#[test]
fn reversed_order_is_tautology() {
    let schema = test_schema_with(&[TwoVariant::schema()]);
    let model_id = TwoVariant::id();

    // Operand order does not matter.
    let expr = ExprOr {
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

    assert!(is_variant_tautology_or(&schema, &expr));
}

#[test]
fn no_is_variant_operands_is_not_tautology() {
    let schema = test_schema_with(&[TwoVariant::schema()]);

    let expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(1)],
    };

    assert!(!is_variant_tautology_or(&schema, &expr));
}
