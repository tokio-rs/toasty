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
/// String`. The sort field is tagged `#[auto]` with `AutoStrategy::String`,
/// matching what `impl Auto for String` produces.
fn make_tenant(id: ModelId) -> Model {
    let mut sk = make_primitive_field(id, 1, "sk", stmt::Type::String, true);
    sk.auto = Some(AutoStrategy::String);
    make_root_with_compound_key(
        id,
        "Tenant",
        vec![
            make_primitive_field(id, 0, "account", stmt::Type::String, true),
            sk,
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
    let mut user_sk = make_primitive_field(user_id, 1, "sk", stmt::Type::String, true);
    user_sk.auto = Some(AutoStrategy::String);
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            user_sk,
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
fn missing_auto_on_sort_field_is_rejected() {
    // Tenant -> User chain where the root's sort field has no `#[auto]`.
    // The validator should reject this with a message that calls out the
    // missing tag.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let mut tenant = make_tenant(tenant_id);
    // Tag the sort field on Tenant with #[auto] using AutoStrategy::String,
    // which is the default `impl Auto for String` produces.
    if let Model::Root(ref mut r) = tenant {
        r.fields[1].auto = Some(AutoStrategy::String);
    }

    // User leaves its sort field with `auto = None` — that's the violation.
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            make_primitive_field(user_id, 1, "sk", stmt::Type::String, true),
        ],
        Some(tenant_id),
    );

    let err = Schema::from_macro(vec![tenant, user]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("must be tagged `#[auto]`"),
        "unexpected error: {msg}",
    );
}

#[test]
fn item_parent_variant_constructs() {
    // B4.6: pure type-system scaffolding. A `Field` can carry the new
    // `FieldTy::ItemParent` variant; the variant exposes the target
    // `ModelId` and an `expr_ty`. No semantic behaviour is wired yet —
    // B4.7 swaps macro emission and B4.8/B4.9 wire lowering.
    let parent = ModelId(0);
    let child = ModelId(1);
    let field = Field {
        id: child.field(2),
        name: FieldName {
            app: Some("tenant".into()),
            storage: None,
        },
        ty: FieldTy::ItemParent(ItemParent {
            target: parent,
            expr_ty: stmt::Type::Model(parent),
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        versionable: false,
        deferred: true,
        constraints: vec![],
        variant: None,
    };

    let item_parent = match &field.ty {
        FieldTy::ItemParent(ip) => ip,
        _ => panic!("expected ItemParent"),
    };
    assert_eq!(item_parent.target, parent);
}

#[test]
fn item_parent_synthesis_emits_itemparent_relation() {
    // B4.7 swaps macro emission from `FieldTy::BelongsTo` to
    // `FieldTy::ItemParent`. Build a (Tenant, User) chain by hand mirroring
    // what the macro now produces — User's parent navigation field carries
    // `FieldTy::ItemParent`. After `Schema::from_macro` walks the chain,
    // `link_relations` and `validate_item_collections` must accept the
    // variant, the field's `target` must resolve to Tenant's `ModelId`, and
    // the variant must round-trip out of the resolved schema.
    //
    // R1.5 (B4.9) requires Tenant to declare an inverse `#[has_many]
    // users`, so we use the `make_tenant_with_users_has_many` helper.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let tenant = make_tenant_with_users_has_many(tenant_id, user_id);

    // User: account (partition), sk (sort, #[auto] String → IC sort key),
    // tenant: ItemParent → Tenant.
    let mut user_sk = make_primitive_field(user_id, 1, "sk", stmt::Type::String, true);
    user_sk.auto = Some(AutoStrategy::String);
    let mut tenant_field =
        make_primitive_field(user_id, 2, "tenant", stmt::Type::Model(tenant_id), false);
    tenant_field.deferred = true;
    tenant_field.ty = FieldTy::ItemParent(ItemParent {
        target: tenant_id,
        expr_ty: stmt::Type::Model(tenant_id),
    });
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            user_sk,
            tenant_field,
        ],
        Some(tenant_id),
    );

    let schema = Schema::from_macro(vec![tenant, user])
        .expect("ItemParent-bearing chain should build cleanly");

    // The `tenant` field on User must surface as `FieldTy::ItemParent` with
    // the resolved `target` pointing at Tenant.
    let user_resolved = schema.model(user_id).as_root_unwrap();
    let tenant_idx = user_resolved
        .fields
        .iter()
        .position(|f| f.name.app.as_deref() == Some("tenant"))
        .expect("User has a `tenant` field");
    match &user_resolved.fields[tenant_idx].ty {
        FieldTy::ItemParent(ip) => {
            assert_eq!(
                ip.target, tenant_id,
                "ItemParent target should resolve to Tenant"
            );
        }
        other => panic!("expected FieldTy::ItemParent on User.tenant, found {other:?}"),
    }
}

/// Build a Tenant root with a `#[has_many]` `users` field of `FieldTy::Has`
/// pre-bound to `pair_id` = `User.tenant`. The link-relations pass in
/// `Schema::from_macro` calls `validate_pair` on a non-placeholder pair_id;
/// when the pair turns out to be `ItemParent`, the schema linker still
/// promotes `Has` -> `HasItems` in the post-link promotion pass.
///
/// Hand-construction is the easiest path here because `FieldId::placeholder()`
/// is crate-private; the production macro emits the placeholder, but tests
/// outside the crate can supply the resolved pair_id directly.
fn make_tenant_with_users_has_many(tenant_id: ModelId, user_id: ModelId) -> Model {
    let mut sk = make_primitive_field(tenant_id, 1, "sk", stmt::Type::String, true);
    sk.auto = Some(AutoStrategy::String);
    let users = Field {
        id: tenant_id.field(2),
        name: FieldName {
            app: Some("users".into()),
            storage: None,
        },
        ty: FieldTy::Has(Has {
            target: user_id,
            expr_ty: stmt::Type::list(stmt::Type::Model(user_id)),
            cardinality: Cardinality::Many {
                singular: Name::new("user"),
            },
            // User.tenant lives at index 2 on the User model — the test's
            // User layout puts (account, sk, tenant) at (0, 1, 2).
            pair_id: user_id.field(2),
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        versionable: false,
        deferred: true,
        constraints: vec![],
        variant: None,
    };

    make_root_with_compound_key(
        tenant_id,
        "Tenant",
        vec![
            make_primitive_field(tenant_id, 0, "account", stmt::Type::String, true),
            sk,
            users,
        ],
        None,
    )
}

#[test]
fn has_items_promotion_replaces_has_when_pair_is_item_parent() {
    // (Tenant, User) chain where Tenant declares `users: Has(Many)` and User
    // declares `tenant: ItemParent(Tenant)`. After `Schema::from_macro`
    // walks the chain, the linker must:
    //   1. resolve Tenant.users.pair_id to User.tenant (pair finder accepts
    //      ItemParent), and
    //   2. promote Tenant.users from FieldTy::Has -> FieldTy::HasItems with
    //      the same target / cardinality / pair_id.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let tenant = make_tenant_with_users_has_many(tenant_id, user_id);

    let mut user_sk = make_primitive_field(user_id, 1, "sk", stmt::Type::String, true);
    user_sk.auto = Some(AutoStrategy::String);
    let mut tenant_field =
        make_primitive_field(user_id, 2, "tenant", stmt::Type::Model(tenant_id), false);
    tenant_field.deferred = true;
    tenant_field.ty = FieldTy::ItemParent(ItemParent {
        target: tenant_id,
        expr_ty: stmt::Type::Model(tenant_id),
    });
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            user_sk,
            tenant_field,
        ],
        Some(tenant_id),
    );

    let schema = Schema::from_macro(vec![tenant, user])
        .expect("Has + ItemParent chain should build cleanly");

    let tenant_resolved = schema.model(tenant_id).as_root_unwrap();
    let users_idx = tenant_resolved
        .fields
        .iter()
        .position(|f| f.name.app.as_deref() == Some("users"))
        .expect("Tenant has a `users` field");
    match &tenant_resolved.fields[users_idx].ty {
        FieldTy::HasItems(hi) => {
            assert_eq!(hi.target, user_id, "HasItems target should resolve to User");
            assert!(
                hi.is_many(),
                "Tenant.users carried Many cardinality before promotion"
            );
            assert_eq!(
                hi.pair_id.model, user_id,
                "pair_id resolves to User.tenant (ItemParent)"
            );
            // The pair must be the User.tenant field.
            let pair_field = schema.field(hi.pair_id);
            assert_eq!(
                pair_field.name.app.as_deref(),
                Some("tenant"),
                "pair_id points at User.tenant"
            );
            assert!(
                matches!(pair_field.ty, FieldTy::ItemParent(_)),
                "pair is FieldTy::ItemParent"
            );
        }
        other => panic!("expected FieldTy::HasItems on Tenant.users, found {other:?}"),
    }
}

#[test]
fn item_parent_requires_inverse_has() {
    // (Tenant, User) chain where User declares `#[item_parent]` but Tenant
    // does NOT declare a `#[has_many] users` (the inverse is missing).
    // Schema::from_macro must reject the schema before any DB I/O.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    // Tenant has only the (account, sk) PK — no `users` field at all.
    let tenant = make_tenant(tenant_id);

    let mut user_sk = make_primitive_field(user_id, 1, "sk", stmt::Type::String, true);
    user_sk.auto = Some(AutoStrategy::String);
    let mut tenant_field =
        make_primitive_field(user_id, 2, "tenant", stmt::Type::Model(tenant_id), false);
    tenant_field.deferred = true;
    tenant_field.ty = FieldTy::ItemParent(ItemParent {
        target: tenant_id,
        expr_ty: stmt::Type::Model(tenant_id),
    });
    let user = make_root_with_compound_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            user_sk,
            tenant_field,
        ],
        Some(tenant_id),
    );

    let err = Schema::from_macro(vec![tenant, user]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("model `Tenant` is the target of `#[item_parent]` on `User`")
            && msg.contains("declares no `#[has_many]` or `#[has_one]` field"),
        "unexpected error: {msg}",
    );
}

#[test]
fn simple_form_root_passes_validation() {
    // Tenant uses the simple `#[key(account, sk)]` form: both fields are
    // recorded as partition components, with no local field. The validator
    // should reinterpret this as (partition=account, sort=sk) for the item
    // collection.
    let tenant_id = ModelId(0);
    let user_id = ModelId(1);

    let mut tenant_sk = make_primitive_field(tenant_id, 1, "sk", stmt::Type::String, true);
    tenant_sk.auto = Some(AutoStrategy::String);
    let mut user_sk = make_primitive_field(user_id, 1, "sk", stmt::Type::String, true);
    user_sk.auto = Some(AutoStrategy::String);
    let tenant = make_root_with_simple_form_key(
        tenant_id,
        "Tenant",
        vec![
            make_primitive_field(tenant_id, 0, "account", stmt::Type::String, true),
            tenant_sk,
        ],
        None,
    );
    let user = make_root_with_simple_form_key(
        user_id,
        "User",
        vec![
            make_primitive_field(user_id, 0, "account", stmt::Type::String, true),
            user_sk,
        ],
        Some(tenant_id),
    );

    Schema::from_macro(vec![tenant, user]).expect("simple-form item-collection chain should build");
}
