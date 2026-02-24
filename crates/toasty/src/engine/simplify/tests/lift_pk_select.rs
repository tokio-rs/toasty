use crate as toasty;
use crate::engine::simplify::Simplify;
use crate::model::Register;
use toasty_core::{
    driver::Capability,
    schema::{app, app::FieldId, Builder},
    stmt::{Expr, Query, Value, Values},
};

#[derive(toasty::Model)]
struct User {
    #[key]
    id: i64,
}

/// Creates a schema with a single `User` model containing an `id` primary
/// key field.
fn test_schema() -> (toasty_core::Schema, FieldId) {
    let app_schema =
        app::Schema::from_macro(&[User::schema()]).expect("schema should build from macro");

    let schema = Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema should build");

    let field_id = FieldId {
        model: User::id(),
        index: 0,
    };

    (schema, field_id)
}

#[test]
fn extracts_value_from_key_eq_filter() {
    let (schema, field_id) = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `extract_key_value(select(eq(id, 42))) → 42`
    let filter = Expr::eq(
        Expr::ref_self_field(field_id),
        Expr::Value(Value::from(42i64)),
    );
    let query = Query::new_select(User::id(), filter);
    let result = simplify.extract_key_expr(&[field_id], &query);

    assert!(result.is_some());
    assert!(matches!(result.unwrap(), Expr::Value(Value::I64(42))));
}

#[test]
fn extracts_value_with_reversed_operands() {
    let (schema, field_id) = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `extract_key_value(select(eq(99, id))) → 99`
    let filter = Expr::eq(
        Expr::Value(Value::from(99i64)),
        Expr::ref_self_field(field_id),
    );
    let query = Query::new_select(User::id(), filter);
    let result = simplify.extract_key_expr(&[field_id], &query);

    assert!(result.is_some());
    assert!(matches!(result.unwrap(), Expr::Value(Value::I64(99))));
}

#[test]
fn values_query_returns_none() {
    let (schema, field_id) = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `extract_key_value(values()) → None`
    let query = Query::values(Values::default());
    let result = simplify.extract_key_expr(&[field_id], &query);

    assert!(result.is_none());
}

#[test]
fn composite_key_returns_none() {
    let (schema, field_id) = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `extract_key_value([field1, field2], ...) → None` (composite keys not supported)

    let filter = Expr::eq(
        Expr::ref_self_field(field_id),
        Expr::Value(Value::from(42i64)),
    );
    let query = Query::new_select(User::id(), filter);
    let field_id2 = FieldId {
        model: User::id(),
        index: 1,
    };
    let result = simplify.extract_key_expr(&[field_id, field_id2], &query);

    assert!(result.is_none());
}
