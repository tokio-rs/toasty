use crate::engine::fold::expr_cast::fold_expr_cast;
use toasty_core::{
    schema::app::ModelId,
    stmt::{Expr, ExprCast, Type, Value},
};
use uuid::Uuid;

#[test]
fn uuid_cast_to_string() {
    // `cast(Uuid("129d7fd7-..."), String) → "129d7fd7-..."`
    let uuid = Uuid::parse_str("129d7fd7-cde2-42d1-be4b-5a27b793f93a").unwrap();
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(Value::Uuid(uuid))),
        ty: Type::String,
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::String(s)) = result.unwrap() else {
        panic!("expected the result to be a `Value::String`");
    };
    assert_eq!(s, "129d7fd7-cde2-42d1-be4b-5a27b793f93a");
}

#[test]
fn string_cast_to_uuid() {
    // `cast("129d7fd7-...", Uuid) → Uuid("129d7fd7-...")`
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(Value::from(
            "129d7fd7-cde2-42d1-be4b-5a27b793f93a",
        ))),
        ty: Type::Uuid,
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::Uuid(uuid)) = result.unwrap() else {
        panic!("expected result to be a `Value::Uuid`");
    };
    assert_eq!(uuid.to_string(), "129d7fd7-cde2-42d1-be4b-5a27b793f93a");
}

#[test]
fn non_const_not_simplified() {
    // `cast(arg(0), String)`, non-constant, not simplified
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::arg(0)),
        ty: Type::String,
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_none());
}

#[test]
fn default_passes_through() {
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Default),
        ty: Type::I32,
    };

    assert_eq!(fold_expr_cast(&mut expr), Some(Expr::Default));
}

#[test]
fn null_passes_through() {
    // `cast(null, String) → null`
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(Value::Null)),
        ty: Type::String,
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(val) = result.unwrap() else {
        panic!("expected result to be a `Value`");
    };
    assert!(val.is_null());
}

#[test]
fn document_lowering_cast_skipped() {
    // A `#[document]` lowering cast (source type present) is schema-directed;
    // fold is schema-free and must leave it for the simplifier.
    let mut expr = ExprCast {
        from: Some(Type::Model(ModelId(0))),
        expr: Box::new(Expr::Value(Value::record_from_vec(vec![Value::from(
            "Alice",
        )]))),
        ty: Type::Object,
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_none());
}

#[test]
fn document_raising_cast_skipped() {
    // A `#[document]` raising cast (model-typed target) is schema-directed;
    // fold is schema-free and must leave it for the simplifier.
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(Value::Null)),
        ty: Type::Model(ModelId(0)),
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_none());
}

#[test]
fn string_identity_cast() {
    // `cast("hello", String) → "hello"`
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(Value::from("hello"))),
        ty: Type::String,
    };
    let result = fold_expr_cast(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::String(s)) = result.unwrap() else {
        panic!("expected result to be a `Value::String`");
    };
    assert_eq!(s, "hello");
}
