use toasty_core::schema::app::*;
use toasty_core::schema::Name;
use toasty_core::stmt;

const MISSING: ModelId = ModelId(99);

fn make_id_field(model_id: ModelId) -> Field {
    Field {
        id: model_id.field(0),
        name: FieldName {
            app_name: "id".to_string(),
            storage_name: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::String,
            storage_ty: None,
        }),
        nullable: false,
        primary_key: true,
        auto: None,
        constraints: vec![],
    }
}

fn make_root_model(id: ModelId, name: &str, extra_fields: Vec<Field>) -> Model {
    let mut fields = vec![make_id_field(id)];
    fields.extend(extra_fields);
    Model::Root(ModelRoot {
        id,
        name: Name::new(name),
        fields,
        primary_key: PrimaryKey {
            fields: vec![id.field(0)],
            index: IndexId {
                model: id,
                index: 0,
            },
        },
        table_name: None,
        indices: vec![],
    })
}

fn make_relation_field(model_id: ModelId, index: usize, name: &str, ty: FieldTy) -> Field {
    Field {
        id: model_id.field(index),
        name: FieldName {
            app_name: name.to_string(),
            storage_name: None,
        },
        ty,
        nullable: false,
        primary_key: false,
        auto: None,
        constraints: vec![],
    }
}

fn assert_missing_model_error(err: &toasty_core::Error, model_name: &str, field_name: &str) {
    let msg = err.to_string();
    assert!(
        msg.contains(model_name),
        "error should mention model `{model_name}`, got: {msg}"
    );
    assert!(
        msg.contains(field_name),
        "error should mention field `{field_name}`, got: {msg}"
    );
    assert!(
        msg.contains("not registered"),
        "error should say 'not registered', got: {msg}"
    );
}

#[test]
fn has_many_target_not_registered() {
    let model_a = ModelId(0);
    let models = vec![make_root_model(
        model_a,
        "Conference",
        vec![make_relation_field(
            model_a,
            1,
            "talks",
            FieldTy::HasMany(HasMany {
                target: MISSING,
                expr_ty: stmt::Type::list(stmt::Type::Unknown),
                singular: Name::new("talk"),
                pair: FieldId {
                    model: MISSING,
                    index: 0,
                },
            }),
        )],
    )];

    let err = Schema::from_macro(&models).unwrap_err();
    assert_missing_model_error(&err, "Conference", "talks");
}

#[test]
fn has_one_target_not_registered() {
    let model_a = ModelId(0);
    let models = vec![make_root_model(
        model_a,
        "User",
        vec![make_relation_field(
            model_a,
            1,
            "profile",
            FieldTy::HasOne(HasOne {
                target: MISSING,
                expr_ty: stmt::Type::Unknown,
                pair: FieldId {
                    model: MISSING,
                    index: 0,
                },
            }),
        )],
    )];

    let err = Schema::from_macro(&models).unwrap_err();
    assert_missing_model_error(&err, "User", "profile");
}

#[test]
fn belongs_to_target_not_registered() {
    let model_a = ModelId(0);
    let models = vec![make_root_model(
        model_a,
        "Talk",
        vec![make_relation_field(
            model_a,
            1,
            "speaker",
            FieldTy::BelongsTo(BelongsTo {
                target: MISSING,
                expr_ty: stmt::Type::Unknown,
                pair: None,
                foreign_key: ForeignKey {
                    fields: vec![ForeignKeyField {
                        source: model_a.field(0),
                        target: MISSING.field(0),
                    }],
                },
            }),
        )],
    )];

    let err = Schema::from_macro(&models).unwrap_err();
    assert_missing_model_error(&err, "Talk", "speaker");
}
