//! Verifies the serializer renders `ExprAnyOp` / `ExprAllOp` as
//! `<lhs> <op> ANY(<rhs>)` / `<lhs> <op> ALL(<rhs>)`.
//!
//! Each case wraps the predicate in a minimal `SELECT` statement and asserts
//! the substring appears in the rendered SQL. The test does not exercise the
//! lowering pipeline — it constructs the AST nodes directly so that the
//! serializer is tested in isolation, including operators (e.g. `>`) the
//! engine does not currently emit but that the node generalizes to.

use toasty_core::{
    schema::db::Schema,
    stmt::{self, BinaryOp, Expr, ExprAllOp, ExprAnyOp},
};
use toasty_sql::{Serializer, Statement as SqlStatement};

/// Render a top-level `Expr` by embedding it as the projection of a `VALUES`
/// row, which serializes to `SELECT <expr>`-equivalent SQL without needing a
/// full table schema.
fn render(expr: Expr) -> String {
    let values = stmt::Values::new(vec![Expr::record([expr])]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    let stmt = SqlStatement::from(core_stmt);
    let schema = Schema::default();
    Serializer::postgresql(&schema).serialize(&stmt)
}

#[test]
fn any_op_eq() {
    let expr = ExprAnyOp {
        lhs: Box::new(Expr::arg(0)),
        op: BinaryOp::Eq,
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render(expr);
    assert!(
        sql.contains("$1 = ANY($2)"),
        "expected `$1 = ANY($2)` in: {sql}"
    );
}

#[test]
fn all_op_ne() {
    let expr = ExprAllOp {
        lhs: Box::new(Expr::arg(0)),
        op: BinaryOp::Ne,
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render(expr);
    assert!(
        sql.contains("$1 <> ALL($2)"),
        "expected `$1 <> ALL($2)` in: {sql}"
    );
}

#[test]
fn any_op_gt_generalizes_to_other_operators() {
    // Stage 1 only lowers `IN` / `NOT IN`, but the node carries a `BinaryOp`
    // so future passes can emit comparisons like `expr > ANY(arr)` without
    // changing the serializer.
    let expr = ExprAnyOp {
        lhs: Box::new(Expr::arg(0)),
        op: BinaryOp::Gt,
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render(expr);
    assert!(
        sql.contains("$1 > ANY($2)"),
        "expected `$1 > ANY($2)` in: {sql}"
    );
}
