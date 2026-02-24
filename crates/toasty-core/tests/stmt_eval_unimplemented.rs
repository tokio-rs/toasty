//! Tests for expression variants that either:
//! - Explicitly return Err (Default), or
//! - Panic with todo!() — these are marked #[ignore] and document the
//!   intended behavior so they can be enabled when implemented.

use toasty_core::stmt::{BinaryOp, Expr, ExprOr, Value};

// ---------------------------------------------------------------------------
// Expr::Default — explicitly errors (database must evaluate it)
// ---------------------------------------------------------------------------

#[test]
fn eval_default_is_error() {
    assert!(Expr::Default.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// Expr::Or — todo!(), should eval like AND but short-circuit on true
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Expr::Or eval not yet implemented (panics with todo!)"]
fn eval_or_both_false_is_false() {
    let expr = Expr::or(false, false);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

#[test]
#[ignore = "Expr::Or eval not yet implemented (panics with todo!)"]
fn eval_or_first_true_is_true() {
    let expr = Expr::or(true, false);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
#[ignore = "Expr::Or eval not yet implemented (panics with todo!)"]
fn eval_or_second_true_is_true() {
    let expr = Expr::or(false, true);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
#[ignore = "Expr::Or eval not yet implemented (panics with todo!)"]
fn eval_or_both_true_is_true() {
    // Use ExprOr directly to bypass Expr::or's is_true() smart-collapse.
    let expr = Expr::Or(ExprOr {
        operands: vec![Expr::from(true), Expr::from(true)],
    });
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
#[ignore = "Expr::Or eval not yet implemented (panics with todo!)"]
fn eval_or_short_circuits_on_true() {
    // not(I64) would error if evaluated — but true comes first, so it should
    // short-circuit and never evaluate the second operand.
    let error_if_evaled = Expr::not(Expr::from(99i64));
    let expr = Expr::or(true, error_if_evaled);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
#[ignore = "Expr::Or eval not yet implemented (panics with todo!)"]
fn eval_or_non_bool_operand_is_error() {
    let expr = ExprOr {
        operands: vec![Expr::from(false), Expr::from(1i64)],
    };
    assert!(Expr::Or(expr).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// BinaryOp::IsA — todo!()
// ---------------------------------------------------------------------------

#[test]
#[ignore = "BinaryOp::IsA not yet implemented (panics with todo!)"]
fn eval_binary_op_is_a() {
    // Placeholder: exact semantics TBD once implemented.
    Expr::binary_op(1i64, BinaryOp::IsA, 1i64).eval_const().unwrap();
}

// ---------------------------------------------------------------------------
// Expr::Map with non-list base — todo!() error handling
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Expr::Map with non-list base panics with todo! instead of returning Err"]
fn eval_map_non_list_base_is_error() {
    let expr = Expr::map(42i64, Expr::arg(0usize));
    assert!(expr.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// Remaining unimplemented variants (all hit `_ => todo!()` in eval_ref)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Expr::Any eval not yet implemented (panics with todo!)"]
fn eval_any_not_implemented() {
    // any([true, false]) should return true
    let expr = Expr::any(Expr::list([true, false]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
#[ignore = "Expr::InList eval not yet implemented (panics with todo!)"]
fn eval_in_list_not_implemented() {
    // 2 in [1, 2, 3] should return true
    let expr = Expr::in_list(2i64, Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}
