use toasty_core::driver::Capability;
use toasty_core::schema::app::*;
use toasty_core::schema::db::{IndexOp, IndexScope};
use toasty_core::schema::{Builder, Name};
use toasty_core::stmt;

fn make_field(model_id: ModelId, index: usize, name: &str, versionable: bool) -> Field {
    Field {
        id: model_id.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: if versionable {
                stmt::Type::I64
            } else {
                stmt::Type::String
            },
            storage_ty: None,
            serialize: None,
        }),
        nullable: false,
        primary_key: index == 0,
        auto: None,
        versionable,
        deferred: false,
        constraints: vec![],
        variant: None,
    }
}

fn build_model(fields: Vec<Field>, version_field: Option<FieldId>) -> Model {
    let id = ModelId(0);
    let pk_index_id = IndexId {
        model: id,
        index: 0,
    };
    Model::Root(ModelRoot {
        id,
        name: Name::new("Thing"),
        fields,
        primary_key: PrimaryKey {
            fields: vec![id.field(0)],
            index: pk_index_id,
        },
        table_name: None,
        indices: vec![Index {
            id: pk_index_id,
            fields: vec![IndexField {
                field: id.field(0),
                op: IndexOp::Eq,
                scope: IndexScope::Partition,
            }],
            unique: true,
            primary_key: true,
        }],
        version_field,
    })
}

#[test]
fn rejects_multiple_version_fields() {
    let id = ModelId(0);
    let model = build_model(
        vec![
            make_field(id, 0, "id", false),
            make_field(id, 1, "v1", true),
            make_field(id, 2, "v2", true),
        ],
        Some(id.field(1)),
    );

    let app_schema = Schema::from_macro(vec![model]).expect("app schema should build");
    let err = Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("Thing"),
        "error should mention model name: {msg}"
    );
    assert!(
        msg.contains("versionable"),
        "error should mention versionable: {msg}"
    );
}

#[test]
fn accepts_single_version_field() {
    let id = ModelId(0);
    let model = build_model(
        vec![make_field(id, 0, "id", false), make_field(id, 1, "v", true)],
        Some(id.field(1)),
    );

    let app_schema = Schema::from_macro(vec![model]).expect("app schema should build");
    Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema with one version field should build");
}

#[test]
fn accepts_no_version_field() {
    let id = ModelId(0);
    let model = build_model(
        vec![
            make_field(id, 0, "id", false),
            make_field(id, 1, "name", false),
        ],
        None,
    );

    let app_schema = Schema::from_macro(vec![model]).expect("app schema should build");
    Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema with no version field should build");
}
