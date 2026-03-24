use assert_struct::assert_struct;
use toasty::codegen_support::Register;
use toasty::stmt::IntoStatement;
use toasty_core::stmt::{self as core_stmt, ExprReference};

// ---------------------------------------------------------------------------
// Test model — field indices: id=0, name=1, age=2, active=3
// ---------------------------------------------------------------------------

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,
    name: String,
    age: i64,
    active: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the core `Select` from anything implementing IntoStatement.
fn select(q: impl IntoStatement) -> core_stmt::Select {
    q.into_statement()
        .into_untyped()
        .into_query_unwrap()
        .into_select()
}

/// Extract just the filter expression.
fn filter_expr(q: impl IntoStatement) -> core_stmt::Expr {
    select(q).filter.into_expr()
}

/// Shorthand for a field reference expr.
fn field_ref(index: usize) -> core_stmt::Expr {
    core_stmt::Expr::Reference(ExprReference::Field { nesting: 0, index })
}

/// Shorthand for a string value expr.
fn str_val(s: &str) -> core_stmt::Expr {
    core_stmt::Expr::Value(core_stmt::Value::String(s.into()))
}

/// Shorthand for an i64 value expr.
fn i64_val(n: i64) -> core_stmt::Expr {
    core_stmt::Expr::Value(core_stmt::Value::I64(n))
}

/// Shorthand for a bool value expr.
fn bool_val(b: bool) -> core_stmt::Expr {
    core_stmt::Expr::Value(core_stmt::Value::Bool(b))
}

// ---------------------------------------------------------------------------
// No filter
// ---------------------------------------------------------------------------

#[test]
fn query_all() {
    let sel = select(toasty::query!(User));

    assert_struct!(sel, _ {
        source: core_stmt::Source::Model(_ { via: None, .. }),
        filter: _ { expr: Some(core_stmt::Expr::Value(core_stmt::Value::Bool(true))) },
        returning: core_stmt::Returning::Model { include: [] },
    });
}

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

#[test]
fn filter_eq_string() {
    let expr = filter_expr(toasty::query!(User filter .name == "Alice"));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(1), str_val("Alice")));
}

#[test]
fn filter_ne_string() {
    let expr = filter_expr(toasty::query!(User filter .name != "Bob"));
    assert_eq!(expr, core_stmt::Expr::ne(field_ref(1), str_val("Bob")));
}

#[test]
fn filter_gt() {
    let expr = filter_expr(toasty::query!(User filter .age > 18));
    assert_eq!(expr, core_stmt::Expr::gt(field_ref(2), i64_val(18)));
}

#[test]
fn filter_ge() {
    let expr = filter_expr(toasty::query!(User filter .age >= 21));
    assert_eq!(expr, core_stmt::Expr::ge(field_ref(2), i64_val(21)));
}

#[test]
fn filter_lt() {
    let expr = filter_expr(toasty::query!(User filter .age < 65));
    assert_eq!(expr, core_stmt::Expr::lt(field_ref(2), i64_val(65)));
}

#[test]
fn filter_le() {
    let expr = filter_expr(toasty::query!(User filter .age <= 99));
    assert_eq!(expr, core_stmt::Expr::le(field_ref(2), i64_val(99)));
}

// ---------------------------------------------------------------------------
// Boolean operators
// ---------------------------------------------------------------------------

#[test]
fn filter_and() {
    let expr = filter_expr(toasty::query!(User filter .name == "Alice" and .age > 18));

    assert_eq!(
        expr,
        core_stmt::Expr::and(
            core_stmt::Expr::eq(field_ref(1), str_val("Alice")),
            core_stmt::Expr::gt(field_ref(2), i64_val(18)),
        )
    );
}

#[test]
fn filter_or() {
    let expr = filter_expr(toasty::query!(User filter .name == "Alice" or .name == "Bob"));

    assert_eq!(
        expr,
        core_stmt::Expr::or(
            core_stmt::Expr::eq(field_ref(1), str_val("Alice")),
            core_stmt::Expr::eq(field_ref(1), str_val("Bob")),
        )
    );
}

#[test]
fn filter_not() {
    let expr = filter_expr(toasty::query!(User filter not .active == true));

    assert_eq!(
        expr,
        core_stmt::Expr::not(core_stmt::Expr::eq(field_ref(3), bool_val(true)))
    );
}

// ---------------------------------------------------------------------------
// Parenthesized expressions
// ---------------------------------------------------------------------------

#[test]
fn filter_parens_override_precedence() {
    // Without parens: .name == "A" or (.name == "B" and .age > 18)
    // With parens: (.name == "A" or .name == "B") and .age > 18
    let expr =
        filter_expr(toasty::query!(User filter (.name == "Alice" or .name == "Bob") and .age > 18));

    // The outer node should be AND (parens made OR bind tighter)
    assert_eq!(
        expr,
        core_stmt::Expr::and(
            core_stmt::Expr::or(
                core_stmt::Expr::eq(field_ref(1), str_val("Alice")),
                core_stmt::Expr::eq(field_ref(1), str_val("Bob")),
            ),
            core_stmt::Expr::gt(field_ref(2), i64_val(18)),
        )
    );
}

// ---------------------------------------------------------------------------
// Boolean literals
// ---------------------------------------------------------------------------

#[test]
fn filter_bool_true() {
    let expr = filter_expr(toasty::query!(User filter .active == true));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(3), bool_val(true)));
}

#[test]
fn filter_bool_false() {
    let expr = filter_expr(toasty::query!(User filter .active == false));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(3), bool_val(false)));
}

// ---------------------------------------------------------------------------
// External references
// ---------------------------------------------------------------------------

#[test]
fn filter_external_variable() {
    let name = String::from("Carl");
    let expr = filter_expr(toasty::query!(User filter .name == #name));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(1), str_val("Carl")));
}

#[test]
fn filter_external_expression() {
    fn make_name() -> String {
        String::from("Computed")
    }

    let expr = filter_expr(toasty::query!(User filter .name == #(make_name())));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(1), str_val("Computed")));
}

// ---------------------------------------------------------------------------
// Case-insensitive keywords
// ---------------------------------------------------------------------------

#[test]
fn filter_keyword_uppercase() {
    let expr = filter_expr(toasty::query!(User FILTER .name == "test"));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(1), str_val("test")));
}

#[test]
fn filter_keyword_mixed_case() {
    let expr = filter_expr(toasty::query!(User Filter .name == "test"));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(1), str_val("test")));
}

#[test]
fn and_keyword_uppercase() {
    let expr = filter_expr(toasty::query!(User filter .name == "a" AND .age > 1));

    assert_struct!(expr, core_stmt::Expr::And(_ { operands.len(): 2 }));
}

#[test]
fn or_keyword_uppercase() {
    let expr = filter_expr(toasty::query!(User filter .name == "a" OR .name == "b"));

    assert_struct!(expr, core_stmt::Expr::Or(_ { operands.len(): 2 }));
}

#[test]
fn not_keyword_uppercase() {
    let expr = filter_expr(toasty::query!(User filter NOT .active == true));

    assert_struct!(expr, core_stmt::Expr::Not(_ { .. }));
}

// ---------------------------------------------------------------------------
// Complex compound expressions
// ---------------------------------------------------------------------------

#[test]
fn complex_and_or_not_with_parens() {
    // Precedence: NOT > AND > OR
    // not .active == true and (.name == "Alice" or .age >= 21)
    // → (NOT(.active == true)) AND ((.name == "Alice") OR (.age >= 21))
    let expr = filter_expr(
        toasty::query!(User filter not .active == true and (.name == "Alice" or .age >= 21)),
    );

    assert_eq!(
        expr,
        core_stmt::Expr::and(
            core_stmt::Expr::not(core_stmt::Expr::eq(field_ref(3), bool_val(true))),
            core_stmt::Expr::or(
                core_stmt::Expr::eq(field_ref(1), str_val("Alice")),
                core_stmt::Expr::ge(field_ref(2), i64_val(21)),
            ),
        )
    );
}

#[test]
fn triple_and() {
    let expr =
        filter_expr(toasty::query!(User filter .name == "A" and .age > 0 and .active == true));

    // AND flattens: and(and(a, b), c) → and(a, b, c)
    assert_struct!(expr, core_stmt::Expr::And(_ { operands.len(): 3 }));
}

#[test]
fn triple_or() {
    let expr =
        filter_expr(toasty::query!(User filter .name == "A" or .name == "B" or .name == "C"));

    // OR flattens: or(or(a, b), c) → or(a, b, c)
    assert_struct!(expr, core_stmt::Expr::Or(_ { operands.len(): 3 }));
}

#[test]
fn or_precedence_lower_than_and() {
    // a and b or c → (a and b) or c
    let expr =
        filter_expr(toasty::query!(User filter .name == "A" and .age > 0 or .active == false));

    assert_eq!(
        expr,
        core_stmt::Expr::or(
            core_stmt::Expr::and(
                core_stmt::Expr::eq(field_ref(1), str_val("A")),
                core_stmt::Expr::gt(field_ref(2), i64_val(0)),
            ),
            core_stmt::Expr::eq(field_ref(3), bool_val(false)),
        )
    );
}

#[test]
fn double_not() {
    let expr = filter_expr(toasty::query!(User filter not not .active == true));

    assert_eq!(
        expr,
        core_stmt::Expr::not(core_stmt::Expr::not(core_stmt::Expr::eq(
            field_ref(3),
            bool_val(true)
        )))
    );
}

// ---------------------------------------------------------------------------
// Query-level properties
// ---------------------------------------------------------------------------

#[test]
fn query_all_is_not_single() {
    let query = toasty::query!(User)
        .into_statement()
        .into_untyped()
        .into_query_unwrap();

    assert_struct!(query, _ {
        single: false,
        order_by: None,
        limit: None,
        with: None,
        body: core_stmt::ExprSet::Select(_),
    });
}

#[test]
fn filter_query_is_not_single() {
    let query = toasty::query!(User filter .age > 0)
        .into_statement()
        .into_untyped()
        .into_query_unwrap();

    assert_struct!(query, _ {
        single: false,
        body: core_stmt::ExprSet::Select(_ {
            source: core_stmt::Source::Model(_ { via: None, .. }),
            ..
        }),
        ..
    });
}

// ---------------------------------------------------------------------------
// Integer literals
// ---------------------------------------------------------------------------

#[test]
fn filter_integer_literal() {
    let expr = filter_expr(toasty::query!(User filter .age == 42));
    assert_eq!(expr, core_stmt::Expr::eq(field_ref(2), i64_val(42)));
}

// ---------------------------------------------------------------------------
// Multiple fields in same expression
// ---------------------------------------------------------------------------

#[test]
fn filter_different_fields() {
    let expr = filter_expr(toasty::query!(User filter .id == 1 and .name == "X" and .age > 0));

    assert_eq!(
        expr,
        core_stmt::Expr::and(
            core_stmt::Expr::and(
                core_stmt::Expr::eq(field_ref(0), i64_val(1)),
                core_stmt::Expr::eq(field_ref(1), str_val("X")),
            ),
            core_stmt::Expr::gt(field_ref(2), i64_val(0)),
        )
    );
}

// ---------------------------------------------------------------------------
// Nested parentheses
// ---------------------------------------------------------------------------

#[test]
fn nested_parens() {
    let expr =
        filter_expr(toasty::query!(User filter ((.name == "A" or .name == "B") and .age > 0)));

    // Outer parens are transparent; inner parens group the OR
    assert_eq!(
        expr,
        core_stmt::Expr::and(
            core_stmt::Expr::or(
                core_stmt::Expr::eq(field_ref(1), str_val("A")),
                core_stmt::Expr::eq(field_ref(1), str_val("B")),
            ),
            core_stmt::Expr::gt(field_ref(2), i64_val(0)),
        )
    );
}

// ---------------------------------------------------------------------------
// Source model identity
// ---------------------------------------------------------------------------

#[test]
fn source_model_matches_user_id() {
    let sel = select(toasty::query!(User));

    // Verify the source references the User model specifically
    let source_model = sel.source.as_model_unwrap();
    assert_eq!(source_model.id, <User as Register>::id());
    assert!(source_model.via.is_none());
}

#[test]
fn filter_source_model_matches_user_id() {
    let sel = select(toasty::query!(User filter .name == "test"));

    let source_model = sel.source.as_model_unwrap();
    assert_eq!(source_model.id, <User as Register>::id());
}
