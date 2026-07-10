use super::test_schema_with;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprCast, ExprReference, Type, Value, ValueObject};

/// Schema fixture for the document-cast tests: a model with a `#[document]`
/// embed, returning the schema and the embed's model-level type
/// (`Type::Model`).
fn document_schema() -> (toasty_core::Schema, Type) {
    use crate as toasty;
    use crate::schema::{Embed, Model};

    #[allow(dead_code)]
    #[derive(toasty::Embed)]
    struct Profile {
        name: String,
        age: i64,
    }

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[document]
        profile: Profile,
    }

    let schema = test_schema_with(&[User::schema(), Profile::schema()]);
    let (_, profile_ty) = schema
        .mapping
        .document_columns
        .first()
        .expect("the schema has one document column");
    let profile_ty = profile_ty.clone();
    assert!(matches!(profile_ty, Type::Model(_)));

    (schema, profile_ty)
}

#[test]
fn document_lowering_cast_folds_to_named_object() {
    let (schema, profile_ty) = document_schema();
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `cast(("Alice", 30) as Object, from Model)` → `{name: "Alice", age: 30}`
    let mut expr = ExprCast {
        from: Some(profile_ty),
        expr: Box::new(Expr::Value(Value::record_from_vec(vec![
            Value::from("Alice"),
            Value::I64(30),
        ]))),
        ty: Type::Object,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    let Some(Expr::Value(Value::Object(object))) = result else {
        panic!("expected the result to be a `Value::Object`");
    };
    assert_eq!(
        object.entries,
        vec![
            ("name".to_owned(), Value::from("Alice")),
            ("age".to_owned(), Value::I64(30)),
        ]
    );
}

#[test]
fn document_raising_cast_folds_to_positional_record() {
    let (schema, profile_ty) = document_schema();
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // A wire object raises to the embed's positional record: an absent key
    // (`name`) decodes to Null, a key unknown to the schema is dropped.
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(Value::Object(ValueObject::from_vec(vec![
            ("age".to_owned(), Value::I64(30)),
            ("unknown".to_owned(), Value::from("dropped")),
        ])))),
        ty: profile_ty,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert_eq!(
        result,
        Some(Expr::Value(Value::record_from_vec(vec![
            Value::Null,
            Value::I64(30),
        ])))
    );
}

#[test]
fn document_raising_cast_is_idempotent() {
    let (schema, profile_ty) = document_schema();
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // An engine-computed positional record passes through the raising cast.
    let record = Value::record_from_vec(vec![Value::from("Alice"), Value::I64(30)]);
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Value(record.clone())),
        ty: profile_ty,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert_eq!(result, Some(Expr::Value(record)));
}

#[test]
fn document_cast_on_non_const_kept() {
    let (schema, profile_ty) = document_schema();
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // A non-constant operand cannot fold; the cast survives for a later,
    // post-substitution simplify.
    let mut expr = ExprCast {
        from: Some(profile_ty),
        expr: Box::new(Expr::arg(0)),
        ty: Type::Object,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_none());
}

#[test]
fn redundant_cast_on_field_eliminated() {
    use crate as toasty;
    use crate::schema::Model;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let simplify = simplify.scope(model.as_root_unwrap());

    // `cast(name_field, String) → name_field`
    // The `name` field (index 1) is already String, so the cast is a no-op.
    let field_ref = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(field_ref.clone()),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert_eq!(result, Some(field_ref));
}

#[test]
fn non_redundant_cast_on_field_kept() {
    use crate as toasty;
    use crate::schema::Model;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let simplify = simplify.scope(model.as_root_unwrap());

    // `cast(name_field, I64)` — String field cast to I64, not redundant.
    let mut expr = ExprCast {
        from: None,
        expr: Box::new(Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 1,
        })),
        ty: Type::I64,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_none());
}
