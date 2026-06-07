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

/// A `Vec<embed>` is a document collection *without* the `#[document]`
/// attribute. An embedded collection has no column-expanded form — the element
/// type alone determines document storage — so the attribute is redundant here;
/// it is only needed to force document storage on a *struct* embed (which
/// otherwise column-expands) or to select an encoding via `#[document(text)]`.
/// This round-trips identically to the annotated `vec_struct_create_get` above.
#[driver_test(id(ID), requires(document_collections))]
pub async fn vec_struct_without_attr(t: &mut Test) -> Result<(), BoxError> {
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
        // No `#[document]`: a `Vec<embed>` is a document collection on its own.
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

/// A `#[document]` field round-trips when its model is loaded through a relation
/// `.include()`. Included rows are merged into the parent by a `NestedMerge`, so
/// this exercises the decode-then-merge path — distinct from the other tests,
/// where decoded rows feed the returning projection directly. Documents sit on
/// both sides (`User.settings` and the included `Order.items`), covering decode
/// on the parent and the child of the merge.
#[driver_test(id(ID), requires(document_collections))]
pub async fn document_field_through_include(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Settings {
        theme: String,
        notifications: bool,
    }

    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct LineItem {
        sku: String,
        qty: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        #[document]
        settings: Settings,
        #[has_many]
        orders: toasty::Deferred<Vec<Order>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
        #[document]
        items: Vec<LineItem>,
    }

    let mut db = t.setup_db(models!(User, Order)).await;

    let settings = Settings {
        theme: "dark".into(),
        notifications: true,
    };
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

    let user = toasty::create!(User {
        name: "Alice",
        settings: settings.clone(),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Order {
        user: &user,
        items: items.clone(),
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(User::fields().orders())
        .get(&mut db)
        .await?;

    // The parent's document is decoded as it flows through the merge.
    assert_eq!(loaded.settings, settings);

    // The included child's document is decoded and merged into the parent.
    let orders = loaded.orders.get();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].items, items);

    Ok(())
}

/// A `#[document]` field nested inside a *column-expanded* embed (the outer
/// embed is not `#[document]`, so it flattens to columns). Column expansion
/// lifts the document field to its own top-level `profile_settings` column of
/// `Type::Document`, so it decodes like any other top-level document column —
/// there is no intermediate record in the data-load row to descend through.
/// This is the "document at a nested embed position" case.
#[driver_test(id(ID), requires(document_collections))]
pub async fn document_in_column_expanded_embed(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Settings {
        theme: String,
        level: i64,
    }

    // Column-expanded embed (no `#[document]`) that *contains* a `#[document]`.
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        #[document]
        settings: Settings,
    }

    // A second column-expanded layer, so the document sits two embeds deep:
    // it flattens to `wrapper_profile_settings`.
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Wrapper {
        profile: Profile,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        wrapper: Wrapper,
    }

    let mut db = t.setup_db(models!(Account)).await;

    let wrapper = Wrapper {
        profile: Profile {
            name: "Alice".into(),
            settings: Settings {
                theme: "dark".into(),
                level: 7,
            },
        },
    };
    let account = toasty::create!(Account {
        wrapper: wrapper.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Account::get_by_id(&mut db, &account.id).await?;
    assert_eq!(reloaded.wrapper, wrapper);

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

/// Filtering on jiff temporal leaves inside a `#[document]` embed. Each path
/// lowers to a JSON extraction that is cast back to the native temporal type on
/// the SQL side (`(col->>'k')::timestamptz`, `::date`, …); before that cast was
/// emitted the comparison rendered `text = timestamptz`, which PostgreSQL
/// rejects. Gated to backends with both document storage and native temporal
/// types — the only place the typed-cast extraction applies.
#[driver_test(id(ID), requires(and(document_collections, native_timestamp)))]
pub async fn struct_embed_filter_temporal(t: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;
    use jiff::civil::{DateTime, date, time};

    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Event {
        name: String,
        starts_at: Timestamp,
        on_date: jiff::civil::Date,
        at_time: jiff::civil::Time,
        scheduled: DateTime,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        event: Event,
    }

    let mut db = t.setup_db(models!(Account)).await;

    let t2000 = Timestamp::from_second(946_684_800)?; // 2000-01-01T00:00:00Z
    let t2020 = Timestamp::from_second(1_577_836_800)?; // 2020-01-01T00:00:00Z

    toasty::create!(Account::[
        { event: Event {
            name: "alpha".into(),
            starts_at: t2000,
            on_date: date(2000, 1, 1),
            at_time: time(9, 30, 0, 0),
            scheduled: date(2000, 1, 1).at(9, 30, 0, 0),
        } },
        { event: Event {
            name: "beta".into(),
            starts_at: t2020,
            on_date: date(2020, 6, 15),
            at_time: time(14, 0, 0, 0),
            scheduled: date(2020, 6, 15).at(14, 0, 0, 0),
        } },
    ])
    .exec(&mut db)
    .await?;

    // Equality on a Timestamp leaf.
    let by_ts = Account::filter(Account::fields().event().starts_at().eq(t2000))
        .exec(&mut db)
        .await?;
    assert_eq!(by_ts.len(), 1);
    assert_eq!(by_ts[0].event.name, "alpha");

    // Range on a Timestamp leaf (text extraction would compare lexicographically).
    let after = Account::filter(Account::fields().event().starts_at().gt(t2000))
        .exec(&mut db)
        .await?;
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].event.name, "beta");

    // Equality on Date, Time, and DateTime leaves.
    let by_date = Account::filter(Account::fields().event().on_date().eq(date(2020, 6, 15)))
        .exec(&mut db)
        .await?;
    assert_eq!(by_date.len(), 1);
    assert_eq!(by_date[0].event.name, "beta");

    let by_time = Account::filter(Account::fields().event().at_time().eq(time(9, 30, 0, 0)))
        .exec(&mut db)
        .await?;
    assert_eq!(by_time.len(), 1);
    assert_eq!(by_time[0].event.name, "alpha");

    let by_dt = Account::filter(
        Account::fields()
            .event()
            .scheduled()
            .eq(date(2020, 6, 15).at(14, 0, 0, 0)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(by_dt.len(), 1);
    assert_eq!(by_dt[0].event.name, "beta");

    Ok(())
}

/// A `Timestamp` leaf in a `#[document]` is truncated to microseconds: the SQL
/// temporal types only hold microseconds, and the codec truncates on write to
/// match the drivers' truncating parameter binding so an equality filter on the
/// original nanosecond value still matches the stored row.
#[driver_test(id(ID), requires(and(document_collections, native_timestamp)))]
pub async fn struct_embed_timestamp_truncates_to_micros(t: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Event {
        at: Timestamp,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        event: Event,
    }

    let mut db = t.setup_db(models!(Account)).await;

    // Nanosecond precision beyond what SQL temporal types (microseconds) hold.
    let nanos = Timestamp::from_second(946_684_800)?
        .checked_add(jiff::Span::new().nanoseconds(123_456_789))?;
    let micros = Timestamp::from_second(946_684_800)?
        .checked_add(jiff::Span::new().nanoseconds(123_456_000))?;

    toasty::create!(Account {
        event: Event { at: nanos }
    })
    .exec(&mut db)
    .await?;

    // Filtering on the original nanosecond value still matches: both the stored
    // value and the bound parameter reduce to the same microsecond instant.
    let found = Account::filter(Account::fields().event().at().eq(nanos))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    // The read-back value is the microsecond truncation — the documented loss.
    assert_eq!(found[0].event.at, micros);

    Ok(())
}

/// A `Zoned` leaf in a `#[document]` is rejected at schema-build: jiff renders
/// it with an RFC 9557 `[IANA]` annotation that no SQL backend can parse back,
/// and dropping the annotation would lose the zone identity the type carries.
#[driver_test(id(ID), requires(document_collections))]
pub async fn struct_embed_zoned_document_rejected(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Debug, PartialEq, toasty::Embed)]
    struct Event {
        name: String,
        when: jiff::Zoned,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,
        #[document]
        event: Event,
    }

    match t.try_setup_db(models!(Account)).await {
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("Zoned") && msg.contains("#[document]"),
                "expected schema-build rejection naming the unsupported `Zoned` leaf, got: {msg}"
            );
        }
        Ok(_) => panic!("expected schema build to reject a `Zoned` leaf in a `#[document]`"),
    }

    Ok(())
}
