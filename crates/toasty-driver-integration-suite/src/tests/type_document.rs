//! Tests for `#[document]` storage of embedded types: a bare `#[derive(Embed)]`
//! struct stored as one document column, and a `Vec<T>` of such structs stored
//! as a JSON array of objects (`jsonb` on PostgreSQL, JSON1 text on SQLite).
//! Each struct is encoded as a JSON object keyed by the embed's field names.
//!
//! This increment covers storage, encoding, whole-value CRUD, and nested-path
//! filtering on bare struct embeds. `#[document]` on a `Vec` element predicate
//! (`.any()` / `.all()`), `partial!` containment, and per-element mutation are
//! not yet implemented. Backends without document support are gated out via
//! `requires(document_collections)`; the negative schema-build path has a
//! dedicated `requires(not(document_collections))` test.

use crate::prelude::*;

/// A `#[document] Vec<struct>` round-trips through INSERT and a fresh fetch:
/// the engine encodes each element as a JSON object on the way in and
/// decodes it back to the embed struct on the way out.
#[driver_test(id(ID), requires(document_collections))]
pub async fn vec_struct_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct LineItem {
        sku: String,
        qty: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,
        #[document]
        items: Vec<LineItem>,
    }

    let mut db = t.setup_db(models!(Order)).await;

    let items = vec![
        LineItem {
            sku: "SKU-1".into(),
            qty: 3,
        },
        LineItem {
            sku: "SKU-2".into(),
            qty: 1,
        },
    ];
    let order = toasty::create!(Order {
        items: items.clone()
    })
    .exec(&mut db)
    .await?;

    let reloaded = Order::get_by_id(&mut db, &order.id).await?;
    assert_eq!(reloaded.items, items);

    Ok(())
}

/// An `Option` field inside a document element round-trips both `Some` and
/// `None`. `None` is omitted from the JSON object entirely and decodes back
/// from the missing key.
#[driver_test(id(ID), requires(document_collections))]
pub async fn vec_struct_option_field(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct LineItem {
        sku: String,
        note: Option<String>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,
        #[document]
        items: Vec<LineItem>,
    }

    let mut db = t.setup_db(models!(Order)).await;

    let items = vec![
        LineItem {
            sku: "SKU-1".into(),
            note: Some("gift wrap".into()),
        },
        LineItem {
            sku: "SKU-2".into(),
            note: None,
        },
    ];
    let order = toasty::create!(Order {
        items: items.clone()
    })
    .exec(&mut db)
    .await?;

    let reloaded = Order::get_by_id(&mut db, &order.id).await?;
    assert_eq!(reloaded.items, items);

    Ok(())
}

/// An empty `Vec<struct>` round-trips as an empty JSON array.
#[driver_test(id(ID), requires(document_collections))]
pub async fn vec_struct_empty(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct LineItem {
        sku: String,
        qty: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,
        #[document]
        items: Vec<LineItem>,
    }

    let mut db = t.setup_db(models!(Order)).await;

    let order = toasty::create!(Order {
        items: Vec::<LineItem>::new(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Order::get_by_id(&mut db, &order.id).await?;
    assert!(reloaded.items.is_empty());

    Ok(())
}

/// Whole-value replacement via the update builder: the assignment path
/// encodes the new `Vec<struct>` the same way the INSERT path does.
#[driver_test(id(ID), requires(document_collections))]
pub async fn vec_struct_update_replace(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct LineItem {
        sku: String,
        qty: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,
        #[document]
        items: Vec<LineItem>,
    }

    let mut db = t.setup_db(models!(Order)).await;

    let mut order = toasty::create!(Order {
        items: vec![LineItem {
            sku: "OLD".into(),
            qty: 1,
        }],
    })
    .exec(&mut db)
    .await?;

    let replacement = vec![
        LineItem {
            sku: "NEW-1".into(),
            qty: 5,
        },
        LineItem {
            sku: "NEW-2".into(),
            qty: 9,
        },
    ];
    order
        .update()
        .items(replacement.clone())
        .exec(&mut db)
        .await?;

    let reloaded = Order::get_by_id(&mut db, &order.id).await?;
    assert_eq!(reloaded.items, replacement);

    Ok(())
}

/// A bare `#[document]` struct embed round-trips through INSERT and a fresh
/// fetch: the engine encodes the embed as one JSON object on the way in and
/// decodes it back to the struct on the way out.
#[driver_test(id(ID), requires(document_collections))]
pub async fn struct_embed_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        age: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        profile: Profile,
    }

    let mut db = t.setup_db(models!(Account)).await;

    let profile = Profile {
        name: "Alice".into(),
        age: 30,
    };
    let account = toasty::create!(Account {
        profile: profile.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Account::get_by_id(&mut db, &account.id).await?;
    assert_eq!(reloaded.profile, profile);

    Ok(())
}

/// Whole-value replacement of a bare `#[document]` embed via the update
/// builder.
#[driver_test(id(ID), requires(document_collections))]
pub async fn struct_embed_update_replace(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        age: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        profile: Profile,
    }

    let mut db = t.setup_db(models!(Account)).await;

    let mut account = toasty::create!(Account {
        profile: Profile {
            name: "old".into(),
            age: 1,
        },
    })
    .exec(&mut db)
    .await?;

    let replacement = Profile {
        name: "new".into(),
        age: 99,
    };
    account
        .update()
        .profile(replacement.clone())
        .exec(&mut db)
        .await?;

    let reloaded = Account::get_by_id(&mut db, &account.id).await?;
    assert_eq!(reloaded.profile, replacement);

    Ok(())
}

/// Filtering on scalar fields inside a bare `#[document]` embed. Each path
/// access lowers to a JSON extraction in the WHERE clause: equality on a
/// string leaf, range on a numeric leaf, and `is_none` on an optional leaf
/// (an absent JSON key).
#[driver_test(id(ID), requires(document_collections))]
pub async fn struct_embed_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        age: i64,
        nickname: Option<String>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        profile: Profile,
    }

    let mut db = t.setup_db(models!(Account)).await;

    toasty::create!(Account::[
        { profile: Profile { name: "Alice".into(), age: 30, nickname: Some("ace".into()) } },
        { profile: Profile { name: "Bob".into(), age: 25, nickname: None } },
        { profile: Profile { name: "Carol".into(), age: 40, nickname: Some("cee".into()) } },
    ])
    .exec(&mut db)
    .await?;

    let alice = Account::filter(Account::fields().profile().name().eq("Alice"))
        .exec(&mut db)
        .await?;
    assert_eq!(alice.len(), 1);
    assert_eq!(alice[0].profile.name, "Alice");

    let over_28 = Account::filter(Account::fields().profile().age().gt(28))
        .exec(&mut db)
        .await?;
    assert_eq!(over_28.len(), 2);

    let no_nickname = Account::filter(Account::fields().profile().nickname().is_none())
        .exec(&mut db)
        .await?;
    assert_eq!(no_nickname.len(), 1);
    assert_eq!(no_nickname[0].profile.name, "Bob");

    Ok(())
}

/// Filtering on a field inside a nested `#[document]` embed: the JSON path
/// descends two levels (`profile.address.city`).
#[driver_test(id(ID), requires(document_collections))]
pub async fn struct_embed_filter_nested(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Address {
        city: String,
        zip: String,
    }

    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        address: Address,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        profile: Profile,
    }

    let mut db = t.setup_db(models!(Account)).await;

    toasty::create!(Account::[
        { profile: Profile { name: "Alice".into(), address: Address { city: "Seattle".into(), zip: "98101".into() } } },
        { profile: Profile { name: "Bob".into(), address: Address { city: "Portland".into(), zip: "97201".into() } } },
    ])
    .exec(&mut db)
    .await?;

    let seattle = Account::filter(Account::fields().profile().address().city().eq("Seattle"))
        .exec(&mut db)
        .await?;
    assert_eq!(seattle.len(), 1);
    assert_eq!(seattle[0].profile.name, "Alice");

    Ok(())
}

/// A nested embed field that itself carries `#[document]`. Unlike
/// `struct_embed_filter_nested` (where the nested embed is column-expanded and
/// arrives as `FieldTy::Embedded`), the macro emits this field as a bare
/// `Type::Model`, just like a top-level `#[document]` field. The schema builder
/// must still resolve it to a nested document so the value encodes as a JSON
/// object rather than reaching the codec as an unencodable positional record.
#[driver_test(id(ID), requires(document_collections))]
pub async fn struct_embed_nested_document_field(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Address {
        city: String,
        zip: String,
    }

    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        #[document]
        address: Address,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        profile: Profile,
    }

    let mut db = t.setup_db(models!(Account)).await;

    let profile = Profile {
        name: "Alice".into(),
        address: Address {
            city: "Seattle".into(),
            zip: "98101".into(),
        },
    };
    let account = toasty::create!(Account {
        profile: profile.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Account::get_by_id(&mut db, &account.id).await?;
    assert_eq!(reloaded.profile, profile);

    // The JSON path still descends through the nested document.
    let seattle = Account::filter(Account::fields().profile().address().city().eq("Seattle"))
        .exec(&mut db)
        .await?;
    assert_eq!(seattle.len(), 1);
    assert_eq!(seattle[0].profile.name, "Alice");

    Ok(())
}

/// On a backend without document-collection support, a `#[document]`
/// collection field is rejected at schema build with a clear error message.
#[driver_test(id(ID), requires(not(document_collections)))]
pub async fn vec_struct_unsupported_backend(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct LineItem {
        sku: String,
        qty: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,
        #[document]
        items: Vec<LineItem>,
    }

    let result = t.try_setup_db(models!(Order)).await;
    match result {
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("#[document]") && msg.contains("does not yet support"),
                "expected schema-build rejection naming the unsupported `#[document]` field, \
                 got: {msg}"
            );
        }
        Ok(_) => panic!("expected schema build to reject #[document] field on this backend"),
    }

    Ok(())
}
