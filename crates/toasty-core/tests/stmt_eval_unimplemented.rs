//! Tests for expression variants that either:
//! - Explicitly return Err (Default), or
//! - Panic with todo!() — these are marked #[ignore] and document the
//!   intended behavior so they can be enabled when implemented.

use toasty_core::stmt::Expr;

// ---------------------------------------------------------------------------
// Expr::Default — explicitly errors (database must evaluate it)
// ---------------------------------------------------------------------------

#[test]
fn eval_default_is_error() {
    assert!(Expr::Default.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// Expr::Map with non-list base — todo!() error handling
// ---------------------------------------------------------------------------

#[test]
#[ignore = "non-list base in Expr::Map panics with todo! instead of returning Err"]
fn eval_map_non_list_base_is_error() {
    let expr = Expr::map(42i64, Expr::arg(0usize));
    assert!(expr.eval_const().is_err());
}
