//! Verifies the serializer renders the various `Expr` branches handled in
//! `serializer/expr.rs` — binary ops, logical chains, LIKE / ILIKE,
//! starts-with, COUNT, LAST_INSERT_ID, the array set/length operators with
//! flavor divergence, and `Expr::Default`.
//!
//! Each case wraps the predicate in a minimal `VALUES` row, which serializes
//! to `SELECT <expr>`-equivalent SQL without needing a full table schema.
//! The serializer is exercised in isolation — no lowering pipeline involved.

use std::panic;

use expect_test::expect;
use toasty_core::{
    schema::db::Schema,
    stmt::{
        self, BinaryOp, Expr, ExprAnd, ExprIntersects, ExprIsSuperset, ExprLength, ExprLike,
        ExprOr, FuncCount,
    },
};
use toasty_sql::{Serializer, Statement as SqlStatement};

/// Render a top-level `Expr` by embedding it as the projection of a `VALUES`
/// row, which serializes to `SELECT <expr>`-equivalent SQL without needing a
/// full table schema.
fn make_stmt(expr: Expr) -> SqlStatement {
    let values = stmt::Values::new(vec![Expr::record([expr])]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    SqlStatement::from(core_stmt)
}

fn render_pg(expr: Expr) -> String {
    let stmt = make_stmt(expr);
    let schema = Schema::default();
    Serializer::postgresql(&schema).serialize(&stmt)
}

fn render_mysql(expr: Expr) -> String {
    let stmt = make_stmt(expr);
    let schema = Schema::default();
    Serializer::mysql(&schema).serialize(&stmt)
}

fn render_sqlite(expr: Expr) -> String {
    let stmt = make_stmt(expr);
    let schema = Schema::default();
    Serializer::sqlite(&schema).serialize(&stmt)
}

// ---------- BinaryOp ----------

#[test]
fn binary_op_all_comparison_operators() {
    let expr = Expr::binary_op(Expr::arg(0), BinaryOp::Eq, Expr::arg(1));
    expect!["VALUES ($1 = $2);"].assert_eq(&render_pg(expr));

    let expr = Expr::binary_op(Expr::arg(0), BinaryOp::Ne, Expr::arg(1));
    expect!["VALUES ($1 <> $2);"].assert_eq(&render_pg(expr));

    let expr = Expr::binary_op(Expr::arg(0), BinaryOp::Lt, Expr::arg(1));
    expect!["VALUES ($1 < $2);"].assert_eq(&render_pg(expr));

    let expr = Expr::binary_op(Expr::arg(0), BinaryOp::Le, Expr::arg(1));
    expect!["VALUES ($1 <= $2);"].assert_eq(&render_pg(expr));

    let expr = Expr::binary_op(Expr::arg(0), BinaryOp::Gt, Expr::arg(1));
    expect!["VALUES ($1 > $2);"].assert_eq(&render_pg(expr));

    let expr = Expr::binary_op(Expr::arg(0), BinaryOp::Ge, Expr::arg(1));
    expect!["VALUES ($1 >= $2);"].assert_eq(&render_pg(expr));
}

// ---------- Logical ----------

#[test]
fn and_chain_joins_operands_with_and_keyword() {
    // `Expr::and` short-circuits on `true`, so build `ExprAnd` directly with
    // three non-trivial operands to exercise the delimiter path.
    let expr: Expr = ExprAnd {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::arg(1)),
            Expr::eq(Expr::arg(2), Expr::arg(3)),
            Expr::eq(Expr::arg(4), Expr::arg(5)),
        ],
    }
    .into();
    expect!["VALUES ($1 = $2 AND $3 = $4 AND $5 = $6);"].assert_eq(&render_pg(expr));
}

#[test]
fn or_chain_joins_operands_with_or_keyword() {
    let expr: Expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::arg(1)),
            Expr::eq(Expr::arg(2), Expr::arg(3)),
            Expr::eq(Expr::arg(4), Expr::arg(5)),
        ],
    }
    .into();
    expect!["VALUES ($1 = $2 OR $3 = $4 OR $5 = $6);"].assert_eq(&render_pg(expr));
}

// ---------- LIKE family ----------

#[test]
fn like_renders_with_like_keyword() {
    let expr = Expr::like(Expr::arg(0), Expr::arg(1));
    expect!["VALUES ($1 LIKE $2);"].assert_eq(&render_pg(expr));
}

#[test]
fn like_with_escape_appends_escape_clause() {
    let expr: Expr = ExprLike {
        expr: Box::new(Expr::arg(0)),
        pattern: Box::new(Expr::arg(1)),
        escape: Some('\\'),
        case_insensitive: false,
    }
    .into();
    expect![[r#"VALUES ($1 LIKE $2 ESCAPE '\');"#]].assert_eq(&render_pg(expr));
}

// `ILIKE` is a pass-through to PostgreSQL's native operator. Only PostgreSQL
// has one, so the query-verify pass rejects a case-insensitive `Expr::Like` on
// every other backend (see `engine/verify.rs`); the serializer therefore only
// ever renders `ILIKE`, and only on PostgreSQL. See
// https://github.com/tokio-rs/toasty/issues/802.
#[test]
fn ilike_renders_on_postgresql() {
    let expr = Expr::ilike(Expr::arg(0), Expr::arg(1));
    expect!["VALUES ($1 ILIKE $2);"].assert_eq(&render_pg(expr));
}

#[test]
fn starts_with_uses_postgresql_prefix_operator() {
    let expr = Expr::starts_with(Expr::arg(0), Expr::arg(1));
    expect!["VALUES ($1 ^@ $2);"].assert_eq(&render_pg(expr));
}

#[test]
fn starts_with_panics_on_non_postgresql_flavors() {
    // The lowering pass rewrites `StartsWith` to `LIKE` for MySQL/SQLite;
    // hitting the serializer directly with `StartsWith` is `unreachable!`.
    for render in [
        render_mysql as fn(Expr) -> String,
        render_sqlite as fn(Expr) -> String,
    ] {
        let result = panic::catch_unwind(|| {
            let expr = Expr::starts_with(Expr::arg(0), Expr::arg(1));
            render(expr)
        });
        assert!(
            result.is_err(),
            "expected StartsWith to panic on non-PG flavor"
        );
    }
}

// ---------- COUNT ----------

#[test]
fn count_star_renders_as_count_star() {
    expect!["VALUES (COUNT(*));"].assert_eq(&render_pg(Expr::count_star()));
}

#[test]
fn count_star_with_filter_renders_filter_clause_on_postgresql() {
    let expr: Expr = FuncCount {
        arg: None,
        filter: Some(Box::new(Expr::eq(Expr::arg(0), Expr::arg(1)))),
    }
    .into();
    expect!["VALUES (COUNT(*) FILTER (WHERE $1 = $2));"].assert_eq(&render_pg(expr));
}

#[test]
fn count_star_with_filter_rewrites_to_case_on_mysql() {
    // MySQL does not support `FILTER (WHERE …)` on aggregates; the serializer
    // rewrites it to `COUNT(CASE WHEN … THEN 1 END)`.
    let expr: Expr = FuncCount {
        arg: None,
        filter: Some(Box::new(Expr::eq(Expr::arg(0), Expr::arg(1)))),
    }
    .into();
    expect!["VALUES ROW(COUNT(CASE WHEN ? = ? THEN 1 END));"].assert_eq(&render_mysql(expr));
}

// ---------- LAST_INSERT_ID ----------

#[test]
fn last_insert_id_renders_on_mysql() {
    expect!["VALUES ROW(LAST_INSERT_ID());"].assert_eq(&render_mysql(Expr::last_insert_id()));
}

// ---------- IsSuperset ----------

#[test]
fn is_superset_uses_at_gt_on_postgresql() {
    let expr: Expr = ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    expect!["VALUES ($1 @> $2);"].assert_eq(&render_pg(expr));
}

#[test]
fn is_superset_uses_json_contains_on_mysql() {
    let expr: Expr = ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    expect!["VALUES ROW(JSON_CONTAINS(?, ?));"].assert_eq(&render_mysql(expr));
}

#[test]
fn is_superset_uses_not_exists_on_sqlite() {
    let expr: Expr = ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    expect!["VALUES (NOT EXISTS (SELECT 1 FROM json_each(?2) AS r WHERE r.value NOT IN (SELECT l.value FROM json_each(?1) AS l)));"].assert_eq(&render_sqlite(expr));
}

// ---------- Intersects (overlap) ----------

#[test]
fn intersects_uses_amp_amp_on_postgresql() {
    let expr: Expr = ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    expect!["VALUES ($1 && $2);"].assert_eq(&render_pg(expr));
}

#[test]
fn intersects_uses_json_overlaps_on_mysql() {
    let expr: Expr = ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    expect!["VALUES ROW(JSON_OVERLAPS(?, ?));"].assert_eq(&render_mysql(expr));
}

#[test]
fn intersects_uses_exists_on_sqlite() {
    let expr: Expr = ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    expect!["VALUES (EXISTS (SELECT 1 FROM json_each(?2) AS r WHERE r.value IN (SELECT l.value FROM json_each(?1) AS l)));"].assert_eq(&render_sqlite(expr));
}

// ---------- Length ----------

#[test]
fn length_uses_cardinality_on_postgresql() {
    let expr: Expr = ExprLength {
        expr: Box::new(Expr::arg(0)),
    }
    .into();
    expect!["VALUES (cardinality($1));"].assert_eq(&render_pg(expr));
}

#[test]
fn length_uses_json_length_on_mysql() {
    let expr: Expr = ExprLength {
        expr: Box::new(Expr::arg(0)),
    }
    .into();
    expect!["VALUES ROW(JSON_LENGTH(?));"].assert_eq(&render_mysql(expr));
}

#[test]
fn length_uses_json_array_length_on_sqlite() {
    let expr: Expr = ExprLength {
        expr: Box::new(Expr::arg(0)),
    }
    .into();
    expect!["VALUES (json_array_length(?1));"].assert_eq(&render_sqlite(expr));
}

// ---------- Default ----------

#[test]
fn default_renders_default_on_postgresql_and_mysql_and_null_on_sqlite() {
    expect!["VALUES (DEFAULT);"].assert_eq(&render_pg(Expr::Default));
    expect!["VALUES ROW(DEFAULT);"].assert_eq(&render_mysql(Expr::Default));
    expect!["VALUES (NULL);"].assert_eq(&render_sqlite(Expr::Default));
}
