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
pub async fn deferred_exec_loads_value(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "the long body".to_string(),
    })
    .exec(&mut db)
    .await?;

    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert!(read.body.is_unloaded());

    // The per-field accessor loads on demand and returns the value.
    let body: String = read.body().exec(&mut db).await?;
    assert_eq!("the long body", body);

    // The in-memory record is not mutated by `.exec()`.
    assert!(read.body.is_unloaded());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::deferred_optional_document))]
pub async fn deferred_optional_exec_loads_value(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    // Create with summary set.
    let with_summary = toasty::create!(Document {
        title: "With summary".to_string(),
        summary: "a brief summary".to_string(),
    })
    .exec(&mut db)
    .await?;

    // Create with summary omitted (nullable, so optional).
    let without_summary = toasty::create!(Document {
        title: "No summary".to_string(),
    })
    .exec(&mut db)
    .await?;

    let with = Document::filter_by_id(with_summary.id).get(&mut db).await?;
    assert!(with.summary.is_unloaded());
    let summary: Option<String> = with.summary().exec(&mut db).await?;
    assert_eq!(Some("a brief summary".to_string()), summary);

    let without = Document::filter_by_id(without_summary.id)
        .get(&mut db)
        .await?;
    assert!(without.summary.is_unloaded());
    let summary: Option<String> = without.summary().exec(&mut db).await?;
    assert_eq!(None, summary);

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

        #[deferred]
        body: Lazy<String>,
    }

    let mut db = t.setup_db(models!(Document)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        body: "the long body".to_string(),
    })
    .exec(&mut db)
    .await?;

    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert!(read.body.is_unloaded());

    let body: String = read.body().exec(&mut db).await?;
    assert_eq!("the long body", body);

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
