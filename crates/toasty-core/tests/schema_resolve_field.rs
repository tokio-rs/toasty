use toasty_core::schema::app::*;
use toasty_core::schema::Name;
use toasty_core::stmt;

const USER: ModelId = ModelId(0);
const STATUS_ENUM: ModelId = ModelId(1);
const CONTACT_ENUM: ModelId = ModelId(2);
const ADDRESS: ModelId = ModelId(3);

fn id_field(model: ModelId) -> Field {
    Field {
        id: model.field(0),
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

fn prim_field(model: ModelId, index: usize, name: &str) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app_name: name.to_string(),
            storage_name: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::String,
            storage_ty: None,
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        constraints: vec![],
    }
}

fn embedded_field(model: ModelId, index: usize, name: &str, target: ModelId) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app_name: name.to_string(),
            storage_name: None,
        },
        ty: FieldTy::Embedded(Embedded {
            target,
            expr_ty: stmt::Type::Model(target),
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        constraints: vec![],
    }
}

/// Schema:
///   User { id, name, status: Status, contact: ContactInfo, address: Address }
///   Status = enum { Active(0), Inactive(1) }  (unit variants only)
///   ContactInfo = enum { Email(0, fields: [address]), Phone(1, fields: [number]) }
///   Address = struct { street, city }
fn schema() -> Schema {
    let status = Model::EmbeddedEnum(EmbeddedEnum {
        id: STATUS_ENUM,
        name: Name::new("Status"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Active"),
                discriminant: 0,
                fields: vec![],
            },
            EnumVariant {
                name: Name::new("Inactive"),
                discriminant: 1,
                fields: vec![],
            },
        ],
    });

    let contact = Model::EmbeddedEnum(EmbeddedEnum {
        id: CONTACT_ENUM,
        name: Name::new("ContactInfo"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Email"),
                discriminant: 0,
                fields: vec![prim_field(CONTACT_ENUM, 0, "address")],
            },
            EnumVariant {
                name: Name::new("Phone"),
                discriminant: 1,
                fields: vec![prim_field(CONTACT_ENUM, 1, "number")],
            },
        ],
    });

    let address = Model::EmbeddedStruct(EmbeddedStruct {
        id: ADDRESS,
        name: Name::new("Address"),
        fields: vec![
            prim_field(ADDRESS, 0, "street"),
            prim_field(ADDRESS, 1, "city"),
        ],
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
        ],
        primary_key: PrimaryKey {
            fields: vec![USER.field(0)],
            index: IndexId {
                model: USER,
                index: 0,
            },
        },
        table_name: None,
        indices: vec![],
    });

    Schema::from_macro(&[user, status, contact, address]).unwrap()
}

// === Primitive fields ===

#[test]
fn resolve_primitive_field() {
    let s = schema();
    let root = s.model(USER);

    // User.name => field at index 1
    let field = s
        .resolve_field(root, &stmt::Projection::from([1]))
        .unwrap();
    assert_eq!(field.name.app_name, "name");
}

#[test]
fn resolve_empty_projection_returns_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(s
        .resolve_field(root, &stmt::Projection::identity())
        .is_none());
}

#[test]
fn resolve_out_of_bounds_returns_none() {
    let s = schema();
    let root = s.model(USER);
    assert!(s
        .resolve_field(root, &stmt::Projection::from([99]))
        .is_none());
}

#[test]
fn resolve_project_through_primitive_returns_none() {
    let s = schema();
    let root = s.model(USER);
    // User.name is primitive — projecting further is invalid
    assert!(s
        .resolve_field(root, &stmt::Projection::from([1, 0]))
        .is_none());
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
    assert_eq!(field.name.app_name, "street");

    // User.address.city => [4, 1]
    let field = s
        .resolve_field(root, &stmt::Projection::from([4, 1]))
        .unwrap();
    assert_eq!(field.name.app_name, "city");
}

// === Embedded enum (data-carrying) — valid two-step projection ===

#[test]
fn resolve_data_enum_variant_field() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> Email(disc=0) -> address(field=0) => [3, 0, 0]
    let field = s
        .resolve_field(root, &stmt::Projection::from([3, 0, 0]))
        .unwrap();
    assert_eq!(field.name.app_name, "address");

    // User.contact -> Phone(disc=1) -> number(positional=0) => [3, 1, 0]
    let field = s
        .resolve_field(root, &stmt::Projection::from([3, 1, 0]))
        .unwrap();
    assert_eq!(field.name.app_name, "number");
}

// === Embedded enum — single step is NOT a valid field resolution ===

#[test]
fn resolve_enum_single_step_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> [0] — only a variant discriminant, not a field
    assert!(s
        .resolve_field(root, &stmt::Projection::from([3, 0]))
        .is_none());
}

#[test]
fn resolve_unit_enum_single_step_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.status -> [0] — unit variant, no fields to resolve
    assert!(s
        .resolve_field(root, &stmt::Projection::from([2, 0]))
        .is_none());
}

// === Embedded enum — invalid variant/field indices ===

#[test]
fn resolve_enum_invalid_variant_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> variant disc 99 doesn't exist
    assert!(s
        .resolve_field(root, &stmt::Projection::from([3, 99, 0]))
        .is_none());
}

#[test]
fn resolve_enum_invalid_field_in_variant_returns_none() {
    let s = schema();
    let root = s.model(USER);

    // User.contact -> Email(disc=0) -> field 99 doesn't exist
    assert!(s
        .resolve_field(root, &stmt::Projection::from([3, 0, 99]))
        .is_none());
}

// === resolve() — Resolved::Variant case ===

#[test]
fn resolve_returns_variant_for_enum_discriminant_access() {
    let s = schema();
    let root = s.model(USER);

    // Single step into data-carrying enum — variant discriminant
    let resolved = s
        .resolve(root, &stmt::Projection::from([3, 0]))
        .unwrap();
    assert!(matches!(resolved, Resolved::Variant(v) if v.name.upper_camel_case() == "Email"));

    let resolved = s
        .resolve(root, &stmt::Projection::from([3, 1]))
        .unwrap();
    assert!(matches!(resolved, Resolved::Variant(v) if v.name.upper_camel_case() == "Phone"));

    // Single step into unit enum — variant discriminant
    let resolved = s
        .resolve(root, &stmt::Projection::from([2, 0]))
        .unwrap();
    assert!(matches!(resolved, Resolved::Variant(v) if v.name.upper_camel_case() == "Active"));
}

#[test]
fn resolve_returns_field_for_enum_variant_field() {
    let s = schema();
    let root = s.model(USER);

    // Two steps into data-carrying enum — variant field access
    let resolved = s
        .resolve(root, &stmt::Projection::from([3, 0, 0]))
        .unwrap();
    assert!(matches!(resolved, Resolved::Field(f) if f.name.app_name == "address"));
}

#[test]
fn resolve_field_returns_none_for_variant_only_projection() {
    let s = schema();
    let root = s.model(USER);

    // resolve_field should return None for variant-only projections
    assert!(s
        .resolve_field(root, &stmt::Projection::from([3, 0]))
        .is_none());
    assert!(s
        .resolve_field(root, &stmt::Projection::from([2, 0]))
        .is_none());
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
    assert!(s
        .resolve(root, &stmt::Projection::from([3, 0, 0]))
        .is_some());
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
