use toasty_core::schema::Name;
use toasty_core::schema::app::*;
use toasty_core::stmt;

const USER: ModelId = ModelId(0);
const STATUS_ENUM: ModelId = ModelId(1);
const CONTACT_ENUM: ModelId = ModelId(2);
const ADDRESS: ModelId = ModelId(3);
const DOC_PROFILE: ModelId = ModelId(4);

fn id_field(model: ModelId) -> Field {
    Field {
        id: model.field(0),
        name: FieldName {
            app: Some("id".to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::String,
            storage_ty: None,
            serialize: None,
        }),
        nullable: false,
        primary_key: true,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: None,
        shared: None,
    }
}

fn prim_field(model: ModelId, index: usize, name: &str) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::String,
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

fn variant_field(model: ModelId, index: usize, name: &str, variant_index: usize) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::String,
            storage_ty: None,
            serialize: None,
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: Some(VariantId {
            model,
            index: variant_index,
        }),
        shared: None,
    }
}

fn embedded_field(model: ModelId, index: usize, name: &str, target: ModelId) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Embedded(Embedded {
            target,
            expr_ty: stmt::Type::Model(target),
            storage_ty: None,
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

/// A `#[document]` field: the target model's fields live inside one document
/// column rather than as `app::Field`s on the parent.
fn document_field(model: ModelId, index: usize, name: &str, target: ModelId) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::Model(target),
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

/// Schema:
///   User { id, name, status: Status, contact: ContactInfo, address: Address,
///          profile: DocProfile as #[document] }
///   Status = enum { Active(0), Inactive(1) }  (unit variants only)
///   ContactInfo = enum { Email(0, fields: [address]), Phone(1, fields: [country_code, number]) }
///   Address = struct { street, city }
///   DocProfile = struct { city, zip, contact: ContactInfo }
///
/// The enum inside `DocProfile` is not a valid Db document shape
/// (`schema::Builder` rejects it), but `Schema::from_macro` accepts it, so
/// the app-schema walk must still resolve paths through it.
fn schema() -> Schema {
    let status = Model::EmbeddedEnum(EmbeddedEnum {
        id: STATUS_ENUM,
        name: Name::new("Status"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Active"),
                discriminant: stmt::Value::I64(0),
                field_range: 0..0,
            },
            EnumVariant {
                name: Name::new("Inactive"),
                discriminant: stmt::Value::I64(1),
                field_range: 0..0,
            },
        ],
        fields: vec![],
        indices: vec![],
    });

    let contact = Model::EmbeddedEnum(EmbeddedEnum {
        id: CONTACT_ENUM,
        name: Name::new("ContactInfo"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Email"),
                discriminant: stmt::Value::I64(0),
                field_range: 0..1,
            },
            EnumVariant {
                name: Name::new("Phone"),
                discriminant: stmt::Value::I64(1),
                field_range: 1..3,
            },
        ],
        fields: vec![
            variant_field(CONTACT_ENUM, 0, "address", 0),
            variant_field(CONTACT_ENUM, 1, "country_code", 1),
            variant_field(CONTACT_ENUM, 2, "number", 1),
        ],
        indices: vec![],
    });

    let address = Model::EmbeddedStruct(EmbeddedStruct {
        id: ADDRESS,
        name: Name::new("Address"),
        fields: vec![
            prim_field(ADDRESS, 0, "street"),
            prim_field(ADDRESS, 1, "city"),
        ],
        indices: vec![],
    });

    let doc_profile = Model::EmbeddedStruct(EmbeddedStruct {
        id: DOC_PROFILE,
        name: Name::new("DocProfile"),
        fields: vec![
            prim_field(DOC_PROFILE, 0, "city"),
            prim_field(DOC_PROFILE, 1, "zip"),
            embedded_field(DOC_PROFILE, 2, "contact", CONTACT_ENUM),
        ],
        indices: vec![],
    });

    let user = Model::Root(ModelRoot {
        id: USER,
        name: Name::new("User"),
        fields: vec![
            id_field(USER),
            prim_field(USER, 1, "name"),
            embedded_field(USER, 2, "status", STATUS_ENUM),
            embedded_field(USER, 3, "contact", CONTACT_ENUM),
            embedded_field(USER, 4, "address", ADDRESS),
            document_field(USER, 5, "profile", DOC_PROFILE),
        ],
        primary_key: PrimaryKey {
            fields: vec![USER.field(0)],
            index: IndexId {
                model: USER,
                index: 0,
            },
        },
        table_name: "users".to_string(),
        indices: vec![],
        version_field: None,
    });

    Schema::from_macro([user, status, contact, address, doc_profile]).unwrap()
}

// === Primitive fields ===

#[test]
fn resolve_primitive_field() {
    let s = schema();
    let root = s.model(USER);

    // User.name => field at index 1
    let field = s.resolve_field(root, &stmt::Projection::from([1])).unwrap();
    assert_eq!(field.name.app.as_deref(), Some("name"));
}

#[test]
fn resolve_empty_projection_returns_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(
        s.resolve_field(root, &stmt::Projection::identity())
            .is_none()
    );
}

#[test]
fn resolve_out_of_bounds_returns_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(
        s.resolve_field(root, &stmt::Projection::from([99]))
            .is_none()
    );
}

#[test]
fn resolve_project_through_primitive_returns_none() {
    let s = schema();
    let root = s.model(USER);
    // User.name is primitive — projecting further is invalid
    assert!(
        s.resolve_field(root, &stmt::Projection::from([1, 0]))
            .is_none()
    );
}

// === Embedded struct ===

#[test]
fn resolve_embedded_struct_field() {
    let s = schema();
    let root = s.model(USER);

    // User.address.street => [4, 0]
    let field = s
        .resolve_field(root, &stmt::Projection::from([4, 0]))
        .unwrap();
    assert_eq!(field.name.app.as_deref(), Some("street"));

    // User.address.city => [4, 1]
    let field = s
        .resolve_field(root, &stmt::Projection::from([4, 1]))
        .unwrap();
    assert_eq!(field.name.app.as_deref(), Some("city"));
}

// === #[document] fields ===

#[test]
fn resolve_document_path_returns_document_field() {
    let s = schema();
    let root = s.model(USER);

    // User.profile.city => the document field, not the sub-field: the
    // document model's fields have no `app::Field` on `User`.
    let field = s
        .resolve_field(root, &stmt::Projection::from([5, 0]))
        .unwrap();
    assert_eq!(field.name.app.as_deref(), Some("profile"));

    // A path that stops at the document field itself resolves to it too.
    let field = s.resolve_field(root, &stmt::Projection::from([5])).unwrap();
    assert_eq!(field.name.app.as_deref(), Some("profile"));
}

// === Embedded enum (data-carrying) — field step is local to the variant ===

#[test]
fn resolve_data_enum_variant_field() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> Email(disc=0) -> address(field=0) => [3, 0, 0]
    let field = s
        .resolve_field(root, &stmt::Projection::from([3, 0, 0]))
        .unwrap();
    assert_eq!(field.name.app.as_deref(), Some("address"));

    // Phone local 0 = country_code => [3, 1, 0]
    let field = s
        .resolve_field(root, &stmt::Projection::from([3, 1, 0]))
        .unwrap();
    assert_eq!(field.name.app.as_deref(), Some("country_code"));

    // Phone local 1 = number => [3, 1, 1]
    let field = s
        .resolve_field(root, &stmt::Projection::from([3, 1, 1]))
        .unwrap();
    assert_eq!(field.name.app.as_deref(), Some("number"));
}

// === Embedded enum — single step is NOT a valid field resolution ===

#[test]
fn resolve_enum_single_step_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> [0] — only a variant discriminant, not a field
    assert!(
        s.resolve_field(root, &stmt::Projection::from([3, 0]))
            .is_none()
    );
}

#[test]
fn resolve_unit_enum_single_step_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.status -> [0] — unit variant, no fields to resolve
    assert!(
        s.resolve_field(root, &stmt::Projection::from([2, 0]))
            .is_none()
    );
}

// === Embedded enum — invalid variant/field indices ===

#[test]
fn resolve_enum_invalid_variant_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> variant disc 99 doesn't exist
    assert!(
        s.resolve_field(root, &stmt::Projection::from([3, 99, 0]))
            .is_none()
    );
}

#[test]
fn resolve_enum_invalid_field_in_variant_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> Email(disc=0) -> field 99 doesn't exist
    assert!(
        s.resolve_field(root, &stmt::Projection::from([3, 0, 99]))
            .is_none()
    );
}

// Field steps must not leak across variants: [3, 0, 1] is Email + Phone's
// field, and [3, 1, 2] is past Phone's two locals.
#[test]
fn resolve_enum_field_index_is_variant_local() {
    let s = schema();
    let root = s.model(USER);

    assert!(
        s.resolve_field(root, &stmt::Projection::from([3, 0, 1]))
            .is_none()
    );
    assert!(
        s.resolve_field(root, &stmt::Projection::from([3, 1, 2]))
            .is_none()
    );
}

// === resolve() — Resolved::Variant case ===

#[test]
fn resolve_returns_variant_for_enum_discriminant_access() {
    let s = schema();
    let root = s.model(USER);

    // Single step into data-carrying enum — variant discriminant
    let resolved = s.resolve(root, &stmt::Projection::from([3, 0])).unwrap();
    assert!(matches!(resolved, Resolved::Variant(v) if v.name.upper_camel_case() == "Email"));

    let resolved = s.resolve(root, &stmt::Projection::from([3, 1])).unwrap();
    assert!(matches!(resolved, Resolved::Variant(v) if v.name.upper_camel_case() == "Phone"));

    // Single step into unit enum — variant discriminant
    let resolved = s.resolve(root, &stmt::Projection::from([2, 0])).unwrap();
    assert!(matches!(resolved, Resolved::Variant(v) if v.name.upper_camel_case() == "Active"));
}

#[test]
fn resolve_returns_field_for_enum_variant_field() {
    let s = schema();
    let root = s.model(USER);

    // Two steps into data-carrying enum — variant field access
    let resolved = s.resolve(root, &stmt::Projection::from([3, 0, 0])).unwrap();
    assert!(matches!(resolved, Resolved::Field(f) if f.name.app.as_deref() == Some("address")));
}

#[test]
fn resolve_field_returns_none_for_variant_only_projection() {
    let s = schema();
    let root = s.model(USER);

    // resolve_field should return None for variant-only projections
    assert!(
        s.resolve_field(root, &stmt::Projection::from([3, 0]))
            .is_none()
    );
    assert!(
        s.resolve_field(root, &stmt::Projection::from([2, 0]))
            .is_none()
    );
}

// === resolve_field_path() — variant roots ===

/// A typed variant path (`Variant(ContactInfo/Phone)` + local step) and its
/// flattened spelling (`Model(User)` + `[3, 1, 1]`) resolve to the same field.
#[test]
fn resolve_field_path_variant_root_matches_flattened_path() {
    let s = schema();

    let mut variant = stmt::Path::from_variant(
        stmt::Path::field(USER, 3),
        VariantId {
            model: CONTACT_ENUM,
            index: 1,
        },
    );
    variant.projection.push(1);

    let mut flattened = stmt::Path::from_index(USER, 3);
    flattened.projection.push(1);
    flattened.projection.push(1);

    let variant_field = s.resolve_field_path(&variant).unwrap();
    let flattened_field = s.resolve_field_path(&flattened).unwrap();
    assert_eq!(variant_field.name.app.as_deref(), Some("number"));
    assert_eq!(variant_field.id, flattened_field.id);
}

#[test]
fn resolve_field_path_variant_root_without_local_step_is_none() {
    let s = schema();

    // A variant root with no local step names a discriminant, not a field.
    let variant = stmt::Path::from_variant(
        stmt::Path::field(USER, 3),
        VariantId {
            model: CONTACT_ENUM,
            index: 1,
        },
    );
    assert!(s.resolve_field_path(&variant).is_none());
}

// === ModelSet::resolve_path() — typed path metadata dialect ===

fn model_set(schema: &Schema) -> ModelSet {
    let mut models = ModelSet::new();
    for model in schema.models.values() {
        models.add(model.clone());
    }
    models
}

#[test]
fn resolve_path_returns_leaf_and_document() {
    let s = schema();
    let models = model_set(&s);

    // User.profile.city => leaf `city`, first document crossed `profile`.
    let path = stmt::Path {
        root: stmt::PathRoot::Model(USER),
        projection: stmt::Projection::from([5, 0]),
    };
    let (leaf, document) = models.resolve_path(&path).unwrap();
    assert_eq!(leaf.name.app.as_deref(), Some("city"));
    assert_eq!(document.unwrap().name.app.as_deref(), Some("profile"));

    // A path that stops at the document field does not cross it.
    let path = stmt::Path {
        root: stmt::PathRoot::Model(USER),
        projection: stmt::Projection::from([5]),
    };
    let (leaf, document) = models.resolve_path(&path).unwrap();
    assert_eq!(leaf.name.app.as_deref(), Some("profile"));
    assert!(document.is_none());
}

#[test]
fn resolve_path_variant_root_resolves_local_field() {
    let s = schema();
    let models = model_set(&s);

    let mut variant = stmt::Path::from_variant(
        stmt::Path::field(USER, 3),
        VariantId {
            model: CONTACT_ENUM,
            index: 1,
        },
    );
    variant.projection.push(1);

    let (leaf, document) = models.resolve_path(&variant).unwrap();
    assert_eq!(leaf.name.app.as_deref(), Some("number"));
    assert!(document.is_none());
}

#[test]
fn resolve_path_variant_root_crossing_document_reports_document() {
    let s = schema();
    let models = model_set(&s);

    // User.profile.contact -> Email -> local 0 (address): the parent path
    // crosses the `profile` document field.
    let mut variant = stmt::Path::from_variant(
        stmt::Path {
            root: stmt::PathRoot::Model(USER),
            projection: stmt::Projection::from([5, 2]),
        },
        VariantId {
            model: CONTACT_ENUM,
            index: 0,
        },
    );
    variant.projection.push(0);

    let (leaf, document) = models.resolve_path(&variant).unwrap();
    assert_eq!(leaf.name.app.as_deref(), Some("address"));
    assert_eq!(document.unwrap().name.app.as_deref(), Some("profile"));

    // `Schema::resolve_field_path` reports the leaf, not the document field.
    let field = s.resolve_field_path(&variant).unwrap();
    assert_eq!(field.name.app.as_deref(), Some("address"));
}

#[test]
fn resolve_path_variant_root_with_mismatched_enum_model_is_err() {
    let s = schema();
    let models = model_set(&s);

    // The parent field is `ContactInfo`, but the variant id names `Status`.
    // Without the check, the local step would be read against `ContactInfo`.
    let mut variant = stmt::Path::from_variant(
        stmt::Path::field(USER, 3),
        VariantId {
            model: STATUS_ENUM,
            index: 0,
        },
    );
    variant.projection.push(0);

    assert!(matches!(
        models.resolve_path(&variant),
        Err(ResolveError::NotEmbeddedEnum { .. })
    ));
    assert!(s.resolve_field_path(&variant).is_none());
}

// === resolve() covers all old is_valid_projection cases ===

#[test]
fn resolve_primitive_field_is_some() {
    let s = schema();
    let root = s.model(USER);
    assert!(s.resolve(root, &stmt::Projection::from([1])).is_some());
}

#[test]
fn resolve_enum_discriminant_is_some() {
    let s = schema();
    let root = s.model(USER);
    assert!(s.resolve(root, &stmt::Projection::from([3, 0])).is_some());
    assert!(s.resolve(root, &stmt::Projection::from([2, 0])).is_some());
}

#[test]
fn resolve_enum_variant_field_is_some() {
    let s = schema();
    let root = s.model(USER);
    assert!(
        s.resolve(root, &stmt::Projection::from([3, 0, 0]))
            .is_some()
    );
}

#[test]
fn resolve_empty_is_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(s.resolve(root, &stmt::Projection::identity()).is_none());
}

#[test]
fn resolve_out_of_bounds_is_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(s.resolve(root, &stmt::Projection::from([99])).is_none());
}

#[test]
fn resolve_through_primitive_is_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(s.resolve(root, &stmt::Projection::from([1, 0])).is_none());
}

#[test]
fn variant_field_ranges_include_empty_variants() {
    let mut s = schema();
    s.models.retain(|id, _| *id == CONTACT_ENUM);
    let Model::EmbeddedEnum(model) = s.models.get_mut(&CONTACT_ENUM).unwrap() else {
        unreachable!()
    };
    model.variants.insert(
        1,
        EnumVariant {
            name: Name::new("Empty"),
            discriminant: stmt::Value::I64(2),
            field_range: 1..1,
        },
    );
    model.variants.push(EnumVariant {
        name: Name::new("Other"),
        discriminant: stmt::Value::I64(3),
        field_range: 3..3,
    });
    for field in &mut model.fields[1..] {
        field.variant.as_mut().unwrap().index = 2;
    }
    assert!(model.variant_fields(1).is_empty());
    assert_eq!(model.variant_fields(2)[0].id, CONTACT_ENUM.field(1));
    assert!(model.variant_fields(3).is_empty());
    toasty_core::schema::Builder::new()
        .build(s, &toasty_core::driver::Capability::SQLITE)
        .unwrap();
}

#[test]
fn rejects_invalid_variant_field_ranges() {
    for ranges in [
        [0..1, 2..3],
        [0..1, 0..3],
        [0..1, 1..4],
        [0..1, 1..2],
        [0..2, 2..3],
    ] {
        let mut s = schema();
        s.models.retain(|id, _| *id == CONTACT_ENUM);
        let Model::EmbeddedEnum(model) = s.models.get_mut(&CONTACT_ENUM).unwrap() else {
            unreachable!()
        };
        for (variant, range) in model.variants.iter_mut().zip(ranges) {
            variant.field_range = range;
        }
        let err = toasty_core::schema::Builder::new()
            .build(s, &toasty_core::driver::Capability::SQLITE)
            .unwrap_err();
        assert!(err.to_string().contains("field range"), "{err}");
    }
}
