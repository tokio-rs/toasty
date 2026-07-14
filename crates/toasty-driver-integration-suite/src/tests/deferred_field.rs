use crate::prelude::*;

#[driver_test(id(ID), scenario(crate::scenarios::deferred_document))]
pub async fn default_load_leaves_deferred_unloaded(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "the long body".to_string(),
    })
    .exec(&mut db)
    .await?;

    // Newly created records expose the value just written as loaded.
    assert_eq!("Hello", created.title);
    assert_eq!("the long body", created.body.get());

    // Querying the model leaves the deferred field unloaded.
    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert_eq!("Hello", read.title);
    assert!(read.body.is_unloaded());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_document))]
pub async fn deferred_include_loads_value(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "the long body".to_string(),
    })
    .exec(&mut db)
    .await?;

    // `.include()` of a deferred primitive eagerly loads it as part of the
    // model query — no separate fetch is needed.
    let read = Document::filter_by_id(created.id)
        .include(Document::fields().body())
        .get(&mut db)
        .await?;

    assert!(!read.body.is_unloaded());
    assert_eq!("the long body", read.body.get());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_optional_document))]
pub async fn deferred_optional_include_loads_some(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "With summary".to_string(),
        summary: "a brief summary".to_string(),
    })
    .exec(&mut db)
    .await?;

    let read = Document::filter_by_id(created.id)
        .include(Document::fields().summary())
        .get(&mut db)
        .await?;

    assert!(!read.summary.is_unloaded());
    assert_eq!(&Some("a brief summary".to_string()), read.summary.get());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_optional_document))]
pub async fn deferred_optional_include_loads_none(t: &mut Test) -> Result<()> {
    // A nullable deferred field must distinguish "loaded as NULL" from
    // "unloaded". An eager `.include()` puts the field into the loaded state
    // even when the column value is NULL.
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "No summary".to_string(),
    })
    .exec(&mut db)
    .await?;

    let read = Document::filter_by_id(created.id)
        .include(Document::fields().summary())
        .get(&mut db)
        .await?;

    assert!(!read.summary.is_unloaded());
    assert_eq!(&None, read.summary.get());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_optional_document))]
pub async fn deferred_optional_create_returns_none_loaded(t: &mut Test) -> Result<()> {
    // INSERT...RETURNING bypasses the deferred mask, so the value the caller
    // just supplied (including `None`) must come back loaded — the in-memory
    // record should not be ambiguous with the unloaded state.
    let mut db = setup(t).await;

    let with_some = toasty::create!(Document {
        title: "With summary".to_string(),
        summary: "hello".to_string(),
    })
    .exec(&mut db)
    .await?;

    assert!(!with_some.summary.is_unloaded());
    assert_eq!(&Some("hello".to_string()), with_some.summary.get());

    let with_none = toasty::create!(Document {
        title: "No summary".to_string(),
    })
    .exec(&mut db)
    .await?;

    assert!(!with_none.summary.is_unloaded());
    assert_eq!(&None, with_none.summary.get());

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::deferred_document))]
pub async fn deferred_filter_does_not_load_field(t: &mut Test) -> Result<()> {
    // SQL-only: a bare predicate on the deferred field requires a full table
    // scan. The DDB equivalent is `deferred_pk_filter_does_not_load_field`,
    // which grounds the query on the primary key.
    let mut db = setup(t).await;

    toasty::create!(Document {
        title: "First".to_string(),
        body: "alpha body".to_string(),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Document {
        title: "Second".to_string(),
        body: "beta body".to_string(),
    })
    .exec(&mut db)
    .await?;

    // Filter on the deferred field — the WHERE clause uses it but the SELECT
    // does not project it.
    let docs = Document::filter(Document::fields().body().eq("alpha body".to_string()))
        .exec(&mut db)
        .await?;

    assert_eq!(1, docs.len());
    assert_eq!("First", docs[0].title);
    assert!(docs[0].body.is_unloaded());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_document))]
pub async fn deferred_pk_filter_does_not_load_field(t: &mut Test) -> Result<()> {
    // Same coverage as `deferred_filter_does_not_load_field`, expressed as a
    // PK-grounded query so it runs on DDB. The deferred field appears in the
    // filter but is still left unloaded in the result.
    let mut db = setup(t).await;

    let alpha = toasty::create!(Document {
        title: "First".to_string(),
        body: "alpha body".to_string(),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Document {
        title: "Second".to_string(),
        body: "beta body".to_string(),
    })
    .exec(&mut db)
    .await?;

    // Match on the PK, with the deferred field as an additional filter.
    let matched = Document::filter_by_id(alpha.id)
        .filter(Document::fields().body().eq("alpha body".to_string()))
        .exec(&mut db)
        .await?;

    assert_eq!(1, matched.len());
    assert_eq!("First", matched[0].title);
    assert!(matched[0].body.is_unloaded());

    // The deferred predicate filters the row out when it does not match.
    let missed = Document::filter_by_id(alpha.id)
        .filter(Document::fields().body().eq("beta body".to_string()))
        .exec(&mut db)
        .await?;
    assert!(missed.is_empty());

    Ok(())
}

#[driver_test(id(ID))]
pub async fn deferred_works_through_type_alias(t: &mut Test) -> Result<()> {
    type Lazy<T> = toasty::Deferred<T>;

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,
        body: Lazy<String>,
    }

    let mut db = t.setup_db(models!(Document)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "the long body".to_string(),
    })
    .exec(&mut db)
    .await?;

    let read = Document::filter_by_id(created.id)
        .include(Document::fields().body())
        .get(&mut db)
        .await?;
    assert_eq!("the long body", read.body.get());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_document))]
pub async fn deferred_update_loads_from_unloaded(t: &mut Test) -> Result<()> {
    // The caller supplied the value as part of the update, so the in-memory
    // field becomes loaded — no follow-up fetch is needed.
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "old body".to_string(),
    })
    .exec(&mut db)
    .await?;

    let mut doc = Document::filter_by_id(created.id).get(&mut db).await?;
    assert!(doc.body.is_unloaded());

    doc.update()
        .body("new body".to_string())
        .exec(&mut db)
        .await?;

    assert!(!doc.body.is_unloaded());
    assert_eq!("new body", doc.body.get());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_document))]
pub async fn deferred_update_refreshes_loaded_value(t: &mut Test) -> Result<()> {
    // An already-loaded deferred field is refreshed by the update, matching
    // non-deferred field behavior.
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "old body".to_string(),
    })
    .exec(&mut db)
    .await?;

    let mut doc = Document::filter_by_id(created.id)
        .include(Document::fields().body())
        .get(&mut db)
        .await?;
    assert_eq!("old body", doc.body.get());

    doc.update()
        .body("new body".to_string())
        .exec(&mut db)
        .await?;

    assert!(!doc.body.is_unloaded());
    assert_eq!("new body", doc.body.get());

    Ok(())
}

// ---------- `Deferred<Json<T>>` on a single field ----------
//
// The column is stored as JSON, the in-memory field is `Deferred<Json<T>>`,
// and `T` only implements `serde::{Serialize, Deserialize}` — never
// Toasty's `Load` directly. Each behavior is exercised in isolation
// against the shared scenario.

#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::deferred_json_document)
)]
pub async fn deferred_json_create_returns_loaded(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let initial = Payload {
        name: "users".to_string(),
        version: 1,
    };

    let created = toasty::create!(Repository {
        name: "main".to_string(),
        payload: initial.clone(),
    })
    .exec(&mut db)
    .await?;

    // INSERT...RETURNING echoes the value the caller supplied, so the field
    // comes back already loaded — even though normal SELECTs would skip it.
    assert!(!created.payload.is_unloaded());
    assert_eq!(&initial, &created.payload.get().0);

    Ok(())
}

#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::deferred_json_document)
)]
pub async fn deferred_json_default_load_leaves_unloaded(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let created = toasty::create!(Repository {
        name: "main".to_string(),
        payload: Payload {
            name: "users".to_string(),
            version: 1,
        },
    })
    .exec(&mut db)
    .await?;

    let read = Repository::filter_by_id(created.id).get(&mut db).await?;
    assert!(read.payload.is_unloaded());

    Ok(())
}

#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::deferred_json_document)
)]
pub async fn deferred_json_include_eager_loads_value(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let initial = Payload {
        name: "users".to_string(),
        version: 1,
    };

    let created = toasty::create!(Repository {
        name: "main".to_string(),
        payload: initial.clone(),
    })
    .exec(&mut db)
    .await?;

    // `.include()` projects the JSON column into the SELECT; the model
    // loader peels the deferred envelope, JSON-decodes the inner String,
    // and wraps the resulting `Json<Payload>` back in a loaded `Deferred`.
    let read = Repository::filter_by_id(created.id)
        .include(Repository::fields().payload())
        .get(&mut db)
        .await?;
    assert!(!read.payload.is_unloaded());
    assert_eq!(&initial, &read.payload.get().0);

    Ok(())
}

#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::deferred_json_document)
)]
pub async fn deferred_json_update_refreshes_loaded_value(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let initial = Payload {
        name: "users".to_string(),
        version: 1,
    };
    let next = Payload {
        name: "users".to_string(),
        version: 2,
    };

    let created = toasty::create!(Repository {
        name: "main".to_string(),
        payload: initial,
    })
    .exec(&mut db)
    .await?;

    let mut doc = Repository::filter_by_id(created.id).get(&mut db).await?;
    assert!(doc.payload.is_unloaded());

    // The update echoes the assigned value back through the reload path,
    // which JSON-decodes and re-wraps in `Deferred`.
    doc.update().payload(next.clone()).exec(&mut db).await?;
    assert!(!doc.payload.is_unloaded());
    assert_eq!(&next, &doc.payload.get().0);

    Ok(())
}
