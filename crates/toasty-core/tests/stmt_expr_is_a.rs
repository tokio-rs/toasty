use toasty_core::schema::Name;
use toasty_core::schema::app;
use toasty_core::stmt::{Expr, Type, TypeUnion, Value};

fn make_union(types: Vec<Type>) -> Type {
    let mut union = TypeUnion::new();
    for ty in types {
        union.insert(ty);
    }
    union.simplify()
}

// ---------------------------------------------------------------------------
// Value expressions delegate to Value::is_a
// ---------------------------------------------------------------------------

#[test]
fn value_matches_own_type() {
    assert!(Expr::from(Value::I64(1)).is_a(&(), &Type::I64));
    assert!(Expr::from(Value::from("hi")).is_a(&(), &Type::String));
}

#[test]
fn value_does_not_match_other_type() {
    assert!(!Expr::from(Value::I64(1)).is_a(&(), &Type::String));
    assert!(!Expr::from(Value::Bool(true)).is_a(&(), &Type::Bytes));
}

#[test]
fn null_value_matches_anything() {
    assert!(Expr::null().is_a(&(), &Type::String));
    assert!(Expr::null().is_a(&(), &Type::I64));
    assert!(Expr::null().is_a(&(), &Type::Record(vec![Type::I32])));
}

// ---------------------------------------------------------------------------
// Union targets: any member may match
// ---------------------------------------------------------------------------

#[test]
fn value_matches_union_member() {
    let union_ty = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    assert!(Expr::from(Value::I64(1)).is_a(&(), &union_ty));
}

#[test]
fn value_does_not_match_union_without_member() {
    let union_ty = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    assert!(!Expr::from(Value::from("hi")).is_a(&(), &union_ty));
}

// ---------------------------------------------------------------------------
// Record expressions check field-by-field
// ---------------------------------------------------------------------------

#[test]
fn record_expr_matches_record_type() {
    let expr = Expr::record_from_vec(vec![
        Expr::from(Value::I64(1)),
        Expr::from(Value::from("hi")),
    ]);
    assert!(expr.is_a(&(), &Type::Record(vec![Type::I64, Type::String])));
}

#[test]
fn record_expr_field_mismatch() {
    let expr = Expr::record_from_vec(vec![
        Expr::from(Value::I64(1)),
        Expr::from(Value::from("hi")),
    ]);
    assert!(!expr.is_a(&(), &Type::Record(vec![Type::String, Type::String])));
}

#[test]
fn record_expr_arity_mismatch() {
    let expr = Expr::record_from_vec(vec![Expr::from(Value::I64(1))]);
    assert!(!expr.is_a(&(), &Type::Record(vec![Type::I64, Type::String])));
}

#[test]
fn record_expr_not_a_scalar() {
    let expr = Expr::record_from_vec(vec![Expr::from(Value::I64(1))]);
    assert!(!expr.is_a(&(), &Type::I64));
}

// ---------------------------------------------------------------------------
// List expressions check every item
// ---------------------------------------------------------------------------

#[test]
fn list_expr_matches_list_type() {
    let expr = Expr::list_from_vec(vec![Expr::from(Value::I64(1)), Expr::from(Value::I64(2))]);
    assert!(expr.is_a(&(), &Type::list(Type::I64)));
}

#[test]
fn list_expr_item_mismatch() {
    let expr = Expr::list_from_vec(vec![
        Expr::from(Value::I64(1)),
        Expr::from(Value::from("x")),
    ]);
    assert!(!expr.is_a(&(), &Type::list(Type::I64)));
}

#[test]
fn list_expr_not_a_record() {
    let expr = Expr::list_from_vec(vec![Expr::from(Value::I64(1))]);
    assert!(!expr.is_a(&(), &Type::Record(vec![Type::I64])));
}

// ---------------------------------------------------------------------------
// Expressions with a statically known result type check that type
// ---------------------------------------------------------------------------

#[test]
fn boolean_predicate_is_a_bool() {
    let expr = Expr::eq(Expr::from(Value::I64(1)), Expr::from(Value::I64(2)));
    assert!(expr.is_a(&(), &Type::Bool));
    assert!(!expr.is_a(&(), &Type::String));
}

#[test]
fn is_null_is_a_bool() {
    let expr = Expr::is_null(Expr::from(Value::I64(1)));
    assert!(expr.is_a(&(), &Type::Bool));
    assert!(!expr.is_a(&(), &Type::I64));
}

#[test]
fn unsupported_expr_panics() {
    let result = std::panic::catch_unwind(|| Expr::arg(0).is_a(&(), &Type::I64));
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Document (`Type::Model`) targets
// ---------------------------------------------------------------------------

const PROFILE: app::ModelId = app::ModelId(0);

fn doc_field(model: app::ModelId, index: usize, name: &str, ty: Type) -> app::Field {
    app::Field {
        id: model.field(index),
        name: app::FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: app::FieldTy::Primitive(app::FieldPrimitive {
            ty,
            storage_ty: None,
            serialize: None,
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: None,
        shared: None,
    }
}

/// Schema: Profile = embedded struct { name: String, age: I64 }
fn document_schema() -> app::Schema {
    let profile = app::Model::EmbeddedStruct(app::EmbeddedStruct {
        id: PROFILE,
        name: Name::new("Profile"),
        fields: vec![
            doc_field(PROFILE, 0, "name", Type::String),
            doc_field(PROFILE, 1, "age", Type::I64),
        ],
        indices: vec![],
    });
    app::Schema::from_macro([profile]).unwrap()
}

#[test]
fn matching_record_value_is_a_document() {
    let schema = document_schema();
    let expr = Expr::from(Value::record_from_vec(vec![
        Value::from("alice"),
        Value::I64(30),
    ]));
    assert!(expr.is_a(&schema, &Type::Model(PROFILE)));
}

#[test]
fn wrong_arity_record_value_is_not_a_document() {
    let schema = document_schema();
    let expr = Expr::from(Value::record_from_vec(vec![Value::from("alice")]));
    assert!(!expr.is_a(&schema, &Type::Model(PROFILE)));
}

#[test]
fn wrong_field_type_record_value_is_not_a_document() {
    let schema = document_schema();
    let expr = Expr::from(Value::record_from_vec(vec![
        Value::from("alice"),
        Value::Bool(true),
    ]));
    assert!(!expr.is_a(&schema, &Type::Model(PROFILE)));
}

#[test]
fn null_field_in_record_value_is_a_document() {
    let schema = document_schema();
    let expr = Expr::from(Value::record_from_vec(vec![Value::Null, Value::I64(30)]));
    assert!(expr.is_a(&schema, &Type::Model(PROFILE)));
}

#[test]
fn matching_record_expr_is_a_document() {
    let schema = document_schema();
    let expr = Expr::record_from_vec(vec![
        Expr::from(Value::from("alice")),
        Expr::from(Value::I64(30)),
    ]);
    assert!(expr.is_a(&schema, &Type::Model(PROFILE)));
}

#[test]
fn wrong_field_type_record_expr_is_not_a_document() {
    let schema = document_schema();
    let expr = Expr::record_from_vec(vec![
        Expr::from(Value::from("alice")),
        Expr::from(Value::Bool(true)),
    ]);
    assert!(!expr.is_a(&schema, &Type::Model(PROFILE)));
}

#[test]
fn schema_free_context_accepts_record_vs_document() {
    // Without a schema there is no layout to check against; the pairing is
    // accepted without inspection.
    let expr = Expr::from(Value::record_from_vec(vec![Value::from("alice")]));
    assert!(expr.is_a(&(), &Type::Model(PROFILE)));
}
