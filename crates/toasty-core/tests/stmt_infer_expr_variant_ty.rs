//! Type inference for `Expr::Variant`: a selection's type is the record of
//! the selected variant's field types, without the discriminant, so a
//! projection over it resolves by variant-local position.

use toasty_core::{
    schema::{Name, app::*},
    stmt::{self, Expr, ExprContext, Type},
};

const OBJECT: ModelId = ModelId(0);
const OWNER: ModelId = ModelId(1);
const ENVELOPE: ModelId = ModelId(2);

const OWNER_PARENT: VariantId = VariantId {
    model: OWNER,
    index: 0,
};
const OWNER_EMPTY: VariantId = VariantId {
    model: OWNER,
    index: 1,
};
const ENVELOPE_FIRST: VariantId = VariantId {
    model: ENVELOPE,
    index: 0,
};

fn field(model: ModelId, index: usize, name: &str, ty: FieldTy, variant: Option<usize>) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty,
        nullable: false,
        primary_key: index == 0 && model == OBJECT,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: variant.map(|index| VariantId { model, index }),
        shared: None,
    }
}

fn primitive(ty: Type) -> FieldTy {
    FieldTy::Primitive(FieldPrimitive {
        ty,
        storage_ty: None,
        serialize: None,
    })
}

fn embedded(target: ModelId) -> FieldTy {
    FieldTy::Embedded(Embedded {
        target,
        expr_ty: Type::Model(target),
        storage_ty: None,
    })
}

fn enum_model(id: ModelId, name: &str, variants: [&str; 2], fields: Vec<Field>) -> Model {
    let mut field_start = 0;
    Model::EmbeddedEnum(EmbeddedEnum {
        id,
        name: Name::new(name),
        discriminant: FieldPrimitive {
            ty: Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: variants
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let start = field_start;
                field_start += fields
                    .iter()
                    .filter(|f| f.variant.unwrap().index == index)
                    .count();
                EnumVariant {
                    name: Name::new(name),
                    discriminant: stmt::Value::I64(index as i64),
                    field_range: start..field_start,
                }
            })
            .collect(),
        fields,
        indices: vec![],
    })
}

/// Schema:
///   Object { id, owner: Owner, envelope: Envelope, maybe_owner: Option<Owner> }
///   Owner = enum { Parent(namespace: Uuid, revision: I64), Empty }
///   Envelope = enum { First(owner: Owner), Second(label: String) }
fn schema() -> Schema {
    let owner = enum_model(
        OWNER,
        "Owner",
        ["Parent", "Empty"],
        vec![
            field(OWNER, 0, "namespace", primitive(Type::Uuid), Some(0)),
            field(OWNER, 1, "revision", primitive(Type::I64), Some(0)),
        ],
    );
    let envelope = enum_model(
        ENVELOPE,
        "Envelope",
        ["First", "Second"],
        vec![
            field(ENVELOPE, 0, "owner", embedded(OWNER), Some(0)),
            field(ENVELOPE, 1, "label", primitive(Type::String), Some(1)),
        ],
    );

    let mut maybe_owner = field(OBJECT, 3, "maybe_owner", embedded(OWNER), None);
    maybe_owner.nullable = true;

    let object = Model::Root(ModelRoot {
        id: OBJECT,
        name: Name::new("Object"),
        fields: vec![
            field(OBJECT, 0, "id", primitive(Type::Uuid), None),
            field(OBJECT, 1, "owner", embedded(OWNER), None),
            field(OBJECT, 2, "envelope", embedded(ENVELOPE), None),
            maybe_owner,
        ],
        primary_key: PrimaryKey {
            fields: vec![OBJECT.field(0)],
            index: IndexId {
                model: OBJECT,
                index: 0,
            },
        },
        table_name: "objects".to_string(),
        indices: vec![],
        referenced_fields: vec![],
        version_field: None,
    });

    Schema::from_macro([object, owner, envelope]).unwrap()
}

fn infer(schema: &Schema, expr: &Expr) -> Type {
    let object = schema.model(OBJECT).as_root_unwrap();
    ExprContext::new_with_target(schema, object).infer_expr_ty(expr, &[])
}

fn owner() -> Expr {
    Expr::ref_self_field(OBJECT.field(1))
}

fn envelope() -> Expr {
    Expr::ref_self_field(OBJECT.field(2))
}

#[test]
fn data_variant_is_record_of_its_fields() {
    let s = schema();
    assert_eq!(
        infer(&s, &Expr::variant(owner(), OWNER_PARENT)),
        Type::Record(vec![Type::Uuid, Type::I64])
    );
}

#[test]
fn unit_variant_is_empty_record() {
    let s = schema();
    assert_eq!(
        infer(&s, &Expr::variant(owner(), OWNER_EMPTY)),
        Type::Record(vec![])
    );
}

#[test]
fn projection_uses_variant_local_positions() {
    let s = schema();
    // Position 1 is the second variant field, not the discriminant slot.
    let expr = Expr::project(Expr::variant(owner(), OWNER_PARENT), [1]);
    assert_eq!(infer(&s, &expr), Type::I64);
}

#[test]
fn nullable_enum_field_selects_like_a_required_one() {
    let s = schema();
    let maybe_owner = Expr::ref_self_field(OBJECT.field(3));
    assert_eq!(
        infer(&s, &Expr::variant(maybe_owner, OWNER_PARENT)),
        Type::Record(vec![Type::Uuid, Type::I64])
    );
}

#[test]
fn nested_selection_through_a_variant_field() {
    let s = schema();
    // envelope as First is a record holding the inner enum.
    let first = Expr::variant(envelope(), ENVELOPE_FIRST);
    assert_eq!(infer(&s, &first), Type::Record(vec![Type::Model(OWNER)]));

    // Selecting the inner enum's variant continues into its fields.
    let inner = Expr::variant(Expr::project(first, [0]), OWNER_PARENT);
    assert_eq!(infer(&s, &inner), Type::Record(vec![Type::Uuid, Type::I64]));
    assert_eq!(infer(&s, &Expr::project(inner, [0])), Type::Uuid);
}

#[test]
#[should_panic(expected = "variant selection on the wrong enum")]
fn selection_on_another_enum_is_rejected() {
    let s = schema();
    // `owner` is an `Owner`; selecting an `Envelope` variant from it is a
    // type error, not a record of the wrong fields.
    infer(&s, &Expr::variant(owner(), ENVELOPE_FIRST));
}
