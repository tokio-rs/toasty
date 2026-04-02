use super::{test_schema, test_schema_with};
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprCast, ExprReference, Type, Value};
use uuid::Uuid;

#[test]
fn uuid_cast_to_string() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `cast(Uuid("129d7fd7-..."), String) → "129d7fd7-..."`
    let uuid = Uuid::parse_str("129d7fd7-cde2-42d1-be4b-5a27b793f93a").unwrap();
    let mut expr = ExprCast {
        expr: Box::new(Expr::Value(Value::Uuid(uuid))),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::String(s)) = result.unwrap() else {
        panic!("expected the result to be a `Value::String`");
    };
    assert_eq!(s, "129d7fd7-cde2-42d1-be4b-5a27b793f93a");
}

#[test]
fn string_cast_to_uuid() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `cast("129d7fd7-...", Uuid) → Uuid("129d7fd7-...")`
    let mut expr = ExprCast {
        expr: Box::new(Expr::Value(Value::from(
            "129d7fd7-cde2-42d1-be4b-5a27b793f93a",
        ))),
        ty: Type::Uuid,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::Uuid(uuid)) = result.unwrap() else {
        panic!("expected result to be a `Value::Uuid`");
    };
    assert_eq!(uuid.to_string(), "129d7fd7-cde2-42d1-be4b-5a27b793f93a");
}

#[test]
fn non_const_not_simplified() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `cast(arg(0), String)`, non-constant, not simplified
    let mut expr = ExprCast {
        expr: Box::new(Expr::arg(0)),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_none());
}

#[test]
fn null_passes_through() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `cast(null, String) → null`
    let mut expr = ExprCast {
        expr: Box::new(Expr::Value(Value::Null)),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(val) = result.unwrap() else {
        panic!("expected result to be a `Value`");
    };
    assert!(val.is_null());
}

#[test]
fn string_identity_cast() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `cast("hello", String) → "hello"`
    let mut expr = ExprCast {
        expr: Box::new(Expr::Value(Value::from("hello"))),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::String(s)) = result.unwrap() else {
        panic!("expected result to be a `Value::String`");
    };
    assert_eq!(s, "hello");
}

#[test]
fn redundant_cast_on_field_eliminated() {
    use crate as toasty;
    use crate::schema::Register;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema);
    let simplify = simplify.scope(model.as_root_unwrap());

    // `cast(name_field, String) → name_field`
    // The `name` field (index 1) is already String, so the cast is a no-op.
    let field_ref = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });
    let mut expr = ExprCast {
        expr: Box::new(field_ref.clone()),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert_eq!(result, Some(field_ref));
}

#[test]
fn non_redundant_cast_on_field_kept() {
    use crate as toasty;
    use crate::schema::Register;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema);
    let simplify = simplify.scope(model.as_root_unwrap());

    // `cast(name_field, I64)` — String field cast to I64, not redundant.
    let mut expr = ExprCast {
        expr: Box::new(Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 1,
        })),
        ty: Type::I64,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_none());
}
