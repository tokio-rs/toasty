//! Tests for `#[document]` collection fields — a `Vec<T>` of an
//! `#[derive(Embed)]` struct, stored as one document column (`jsonb` on
//! PostgreSQL, JSON1 text on SQLite). Each element is encoded as a JSON
//! object keyed by the embed's field names.
//!
//! This increment covers storage, encoding, and whole-value CRUD. Element
//! predicates (`.any()` / `.all()`), `partial!` containment, and per-element
//! mutation are not yet implemented. Backends without document-collection
//! support are gated out via `requires(document_collections)`; the negative
//! schema-build path has a dedicated `requires(not(document_collections))`
//! test.

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
