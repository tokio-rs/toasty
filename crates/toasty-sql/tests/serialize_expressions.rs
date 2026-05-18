//! Verifies the serializer renders the various `Expr` branches handled in
//! `serializer/expr.rs` — binary ops, logical chains, LIKE / ILIKE,
//! starts-with, COUNT, LAST_INSERT_ID, the array set/length operators with
//! flavor divergence, and `Expr::Default`.
//!
//! Each case wraps the predicate in a minimal `VALUES` row, which serializes
//! to `SELECT <expr>`-equivalent SQL without needing a full table schema.
//! The serializer is exercised in isolation — no lowering pipeline involved.

use std::panic;

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
    let cases: &[(BinaryOp, &str)] = &[
        (BinaryOp::Eq, "$1 = $2"),
        (BinaryOp::Ne, "$1 <> $2"),
        (BinaryOp::Lt, "$1 < $2"),
        (BinaryOp::Le, "$1 <= $2"),
        (BinaryOp::Gt, "$1 > $2"),
        (BinaryOp::Ge, "$1 >= $2"),
    ];
    for (op, expected) in cases {
        let expr = Expr::binary_op(Expr::arg(0), *op, Expr::arg(1));
        let sql = render_pg(expr);
        assert!(
            sql.contains(expected),
            "expected `{expected}` for op {op:?} in: {sql}"
        );
    }
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
    let sql = render_pg(expr);
    assert!(
        sql.contains("$1 = $2 AND $3 = $4 AND $5 = $6"),
        "expected three operands joined by AND in: {sql}"
    );
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
    let sql = render_pg(expr);
    assert!(
        sql.contains("$1 = $2 OR $3 = $4 OR $5 = $6"),
        "expected three operands joined by OR in: {sql}"
    );
}

// ---------- LIKE family ----------

#[test]
fn like_renders_with_like_keyword() {
    let expr = Expr::like(Expr::arg(0), Expr::arg(1));
    let sql = render_pg(expr);
    assert!(sql.contains("$1 LIKE $2"), "expected ` LIKE ` in: {sql}");
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
    let sql = render_pg(expr);
    assert!(
        sql.contains("$1 LIKE $2 ESCAPE "),
        "expected ESCAPE clause in: {sql}"
    );
}

#[test]
fn ilike_renders_on_postgresql() {
    let expr = Expr::ilike(Expr::arg(0), Expr::arg(1));
    let sql = render_pg(expr);
    assert!(sql.contains("$1 ILIKE $2"), "expected ` ILIKE ` in: {sql}");
}

// MySQL and SQLite do not have `ILIKE`. The serializer currently emits plain
// `LIKE`, which happens to match case-insensitively for ASCII because of the
// default collations on both engines — but SQLite's `LIKE` is case-sensitive
// for non-ASCII characters, so `.ilike("café%")` silently fails to match
// `CAFÉ`. See https://github.com/tokio-rs/toasty/issues/802.
//
// The tests below pin the expected behavior once the serializer is fixed —
// likely by wrapping both sides in `LOWER(...)` (the most portable option)
// or by adding a `COLLATE` clause. They assert the most-likely shape; if
// the eventual fix lands a different mechanism the assertion should be
// adjusted at that point.

#[ignore = "ilike case-insensitivity for non-ASCII is broken on MySQL/SQLite — see #802"]
#[test]
fn ilike_handles_non_ascii_case_insensitivity_on_mysql() {
    let expr = Expr::ilike(Expr::arg(0), Expr::arg(1));
    let sql = render_mysql(expr);
    assert!(
        sql.contains("LOWER("),
        "expected case-folding wrapper (e.g. LOWER) in: {sql}"
    );
}

#[ignore = "ilike case-insensitivity for non-ASCII is broken on MySQL/SQLite — see #802"]
#[test]
fn ilike_handles_non_ascii_case_insensitivity_on_sqlite() {
    let expr = Expr::ilike(Expr::arg(0), Expr::arg(1));
    let sql = render_sqlite(expr);
    assert!(
        sql.contains("LOWER("),
        "expected case-folding wrapper (e.g. LOWER) in: {sql}"
    );
}

#[test]
fn starts_with_uses_postgresql_prefix_operator() {
    let expr = Expr::starts_with(Expr::arg(0), Expr::arg(1));
    let sql = render_pg(expr);
    assert!(sql.contains("$1 ^@ $2"), "expected `^@` in: {sql}");
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
    let sql = render_pg(Expr::count_star());
    assert!(sql.contains("COUNT(*)"), "expected COUNT(*) in: {sql}");
}

#[test]
fn count_star_with_filter_renders_filter_clause_on_postgresql() {
    let expr: Expr = FuncCount {
        arg: None,
        filter: Some(Box::new(Expr::eq(Expr::arg(0), Expr::arg(1)))),
    }
    .into();
    let sql = render_pg(expr);
    assert!(
        sql.contains("COUNT(*) FILTER (WHERE $1 = $2)"),
        "expected FILTER clause in: {sql}"
    );
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
    let sql = render_mysql(expr);
    assert!(
        sql.contains("COUNT(CASE WHEN ? = ? THEN 1 END)"),
        "expected CASE rewrite in: {sql}"
    );
}

// ---------- LAST_INSERT_ID ----------

#[test]
fn last_insert_id_renders_on_mysql() {
    let sql = render_mysql(Expr::last_insert_id());
    assert!(
        sql.contains("LAST_INSERT_ID()"),
        "expected LAST_INSERT_ID() in: {sql}"
    );
}

// ---------- IsSuperset ----------

#[test]
fn is_superset_uses_at_gt_on_postgresql() {
    let expr: Expr = ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render_pg(expr);
    assert!(sql.contains("$1 @> $2"), "expected `@>` in: {sql}");
}

#[test]
fn is_superset_uses_json_contains_on_mysql() {
    let expr: Expr = ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render_mysql(expr);
    assert!(
        sql.contains("JSON_CONTAINS("),
        "expected JSON_CONTAINS( in: {sql}"
    );
}

#[test]
fn is_superset_uses_not_exists_on_sqlite() {
    let expr: Expr = ExprIsSuperset {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render_sqlite(expr);
    assert!(
        sql.contains("NOT EXISTS ("),
        "expected NOT EXISTS ( in: {sql}"
    );
}

// ---------- Intersects (overlap) ----------

#[test]
fn intersects_uses_amp_amp_on_postgresql() {
    let expr: Expr = ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render_pg(expr);
    assert!(sql.contains("$1 && $2"), "expected `&&` in: {sql}");
}

#[test]
fn intersects_uses_json_overlaps_on_mysql() {
    let expr: Expr = ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render_mysql(expr);
    assert!(
        sql.contains("JSON_OVERLAPS("),
        "expected JSON_OVERLAPS( in: {sql}"
    );
}

#[test]
fn intersects_uses_exists_on_sqlite() {
    let expr: Expr = ExprIntersects {
        lhs: Box::new(Expr::arg(0)),
        rhs: Box::new(Expr::arg(1)),
    }
    .into();
    let sql = render_sqlite(expr);
    assert!(sql.contains("EXISTS ("), "expected EXISTS ( in: {sql}");
    assert!(
        !sql.contains("NOT EXISTS ("),
        "Intersects should not emit NOT EXISTS on SQLite in: {sql}"
    );
}

// ---------- Length ----------

#[test]
fn length_uses_cardinality_on_postgresql() {
    let expr: Expr = ExprLength {
        expr: Box::new(Expr::arg(0)),
    }
    .into();
    let sql = render_pg(expr);
    assert!(
        sql.contains("cardinality($1)"),
        "expected cardinality(...) in: {sql}"
    );
}

#[test]
fn length_uses_json_length_on_mysql() {
    let expr: Expr = ExprLength {
        expr: Box::new(Expr::arg(0)),
    }
    .into();
    let sql = render_mysql(expr);
    assert!(
        sql.contains("JSON_LENGTH(?)"),
        "expected JSON_LENGTH(...) in: {sql}"
    );
}

#[test]
fn length_uses_json_array_length_on_sqlite() {
    let expr: Expr = ExprLength {
        expr: Box::new(Expr::arg(0)),
    }
    .into();
    let sql = render_sqlite(expr);
    assert!(
        sql.contains("json_array_length(?1)"),
        "expected json_array_length(...) in: {sql}"
    );
}

// ---------- Default ----------

#[test]
fn default_renders_default_on_postgresql_and_mysql_and_null_on_sqlite() {
    let pg = render_pg(Expr::Default);
    assert!(pg.contains("DEFAULT"), "expected DEFAULT on PG in: {pg}");

    let my = render_mysql(Expr::Default);
    assert!(my.contains("DEFAULT"), "expected DEFAULT on MySQL in: {my}");

    let lite = render_sqlite(Expr::Default);
    assert!(lite.contains("NULL"), "expected NULL on SQLite in: {lite}");
    assert!(
        !lite.contains("DEFAULT"),
        "SQLite should not render DEFAULT in: {lite}"
    );
}
