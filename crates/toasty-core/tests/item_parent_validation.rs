//! Schema-build validation for item-collection symmetric keys.
//!
//! These tests exercise validation that runs at `Schema::from_macro` time —
//! the same path `Db::builder().connect(...)` walks just before talking to
//! a driver. The macro layer cannot perform these checks because each
//! `#[derive(Model)]` invocation sees only its own struct.
//!
//! Models are constructed by hand rather than via the derive macro because
//! the trait wiring that lets `Deferred<Tenant>` compile as a field type
//! (without a `#[belongs_to]`) is synthesised in a later step (A5). The
//! schema-builder validation we exercise here is independent of that wiring.

use toasty_core::schema::Name;
use toasty_core::schema::app::*;
use toasty_core::schema::db::{IndexOp, IndexScope};
use toasty_core::stmt;

/// Build a primitive field with the given name and primitive type.
fn make_primitive_field(
    model_id: ModelId,
    index: usize,
    name: &str,
    ty: stmt::Type,
    primary_key: bool,
) -> Field {
    Field {
        id: model_id.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty,
            storage_ty: None,
            serialize: None,
        }),
        nullable: false,
        primary_key,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: None,
    }
}

/// Build a root with a single (partition, sort) `#[key]`. The first two
/// entries in `fields` (positions 0 and 1) become the partition and sort key
/// respectively. Caller is responsible for marking those fields
/// `primary_key: true`.
fn make_root_with_compound_key(
    id: ModelId,
    name: &str,
    fields: Vec<Field>,
    parent: Option<ModelId>,
) -> Model {
    let pk_index_id = IndexId {
        model: id,
        index: 0,
    };
    Model::Root(ModelRoot {
        id,
        name: Name::new(name),
        fields,
        primary_key: PrimaryKey {
            fields: vec![id.field(0), id.field(1)],
            index: pk_index_id,
        },
        table_name: None,
        parent,
        indices: vec![Index {
            id: pk_index_id,
            name: None,
            fields: vec![
                IndexField {
                    field: id.field(0),
                    op: IndexOp::Eq,
                    scope: IndexScope::Partition,
                },
                IndexField {
                    field: id.field(1),
                    op: IndexOp::Eq,
                    scope: IndexScope::Local,
                },
            ],
            unique: true,
            primary_key: true,
        }],
        version_field: None,
    })
}

/// A canonical Tenant root: `#[key(account, sk)]` with `account: String, sk:
/// String`.
fn make_tenant(id: ModelId) -> Model {
    make_root_with_compound_key(
        id,
        "Tenant",
        vec![
            make_primitive_field(id, 0, "account", stmt::Type::String, true),
            make_primitive_field(id, 1, "sk", stmt::Type::String, true),
        ],
        None,
    )
}

#[test]
fn child_missing_root_partition_field() {
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let tenant = make_tenant(tenant_id);

    // User declares `account_other` instead of `account`, so it does not
    // satisfy the root's partition-key contract.
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account_other", stmt::Type::String, true),
            make_primitive_field(user_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(tenant_id),
    );

    let err = Schema::from_macro(vec![tenant, user]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("expected field `account: String` matching root `Tenant`'s partition key"),
        "unexpected error: {msg}",
    );
}

#[test]
fn child_wrong_partition_type() {
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let tenant = make_tenant(tenant_id);

    // User has the right field name but a `u64` instead of `String`.
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::U64, true),
            make_primitive_field(user_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(tenant_id),
    );

    let err = Schema::from_macro(vec![tenant, user]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("field `account` on `User` has type `u64`; root `Tenant` declares `String`"),
        "unexpected error: {msg}",
    );
}

#[test]
fn root_sort_field_must_be_string() {
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    // Tenant declares a non-String sort key.
    let tenant = make_root_with_compound_key(
        tenant_id,
        "Tenant",
        vec![
            make_primitive_field(tenant_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(tenant_id, 1, "sk", stmt::Type::Uuid, true),
        ],
        None,
    );

    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(user_id, 1, "sk", stmt::Type::Uuid, true),
        ],
        Some(tenant_id),
    );

    let err = Schema::from_macro(vec![tenant, user]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("sort field `sk` must be `String`; found `Uuid`"),
        "unexpected error: {msg}",
    );
}

#[test]
fn cycle_detected() {
    // Two children that point at each other — neither has a true root.
    let a_id = ModelId(0);
    let b_id = ModelId(1);

    let a = make_root_with_compound_key(
        a_id,
        "A",
        vec![
            make_primitive_field(a_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(a_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(b_id),
    );
    let b = make_root_with_compound_key(
        b_id,
        "B",
        vec![
            make_primitive_field(b_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(b_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(a_id),
    );

    let err = Schema::from_macro(vec![a, b]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("item-collection cycle detected"),
        "unexpected error: {msg}",
    );
}

#[test]
fn well_formed_chain_passes() {
    // Tenant -> User: User correctly mirrors Tenant's PK fields.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let tenant = make_tenant(tenant_id);
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(user_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(tenant_id),
    );

    Schema::from_macro(vec![tenant, user]).expect("well-formed item-collection chain should build");
}

/// Build a root whose `#[key(a, b)]` was parsed in simple form: both fields
/// land in the partition vector, no local fields. This mirrors the macro's
/// output today for a bare `#[key(account, sk)]` attribute. The validator is
/// expected to reinterpret position 0 as partition and position 1 as sort
/// when this model participates in an item collection.
fn make_root_with_simple_form_key(
    id: ModelId,
    name: &str,
    fields: Vec<Field>,
    parent: Option<ModelId>,
) -> Model {
    let pk_index_id = IndexId {
        model: id,
        index: 0,
    };
    Model::Root(ModelRoot {
        id,
        name: Name::new(name),
        fields,
        primary_key: PrimaryKey {
            fields: vec![id.field(0), id.field(1)],
            index: pk_index_id,
        },
        table_name: None,
        parent,
        indices: vec![Index {
            id: pk_index_id,
            name: None,
            fields: vec![
                IndexField {
                    field: id.field(0),
                    op: IndexOp::Eq,
                    scope: IndexScope::Partition,
                },
                IndexField {
                    field: id.field(1),
                    op: IndexOp::Eq,
                    scope: IndexScope::Partition,
                },
            ],
            unique: true,
            primary_key: true,
        }],
        version_field: None,
    })
}

#[test]
fn simple_form_root_passes_validation() {
    // Tenant uses the simple `#[key(account, sk)]` form: both fields are
    // recorded as partition components, with no local field. The validator
    // should reinterpret this as (partition=account, sort=sk) for the item
    // collection.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let tenant = make_root_with_simple_form_key(
        tenant_id,
        "Tenant",
        vec![
            make_primitive_field(tenant_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(tenant_id, 1, "sk", stmt::Type::String, true),
        ],
        None,
    );
    let user = make_root_with_simple_form_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(user_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(tenant_id),
    );

    Schema::from_macro(vec![tenant, user]).expect("simple-form item-collection chain should build");
}
