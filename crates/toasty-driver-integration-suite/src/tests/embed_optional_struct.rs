//! `Option<EmbeddedType>` model fields: a nullable embed flattens to nullable
//! columns and round-trips `None` (all columns NULL) ⇄ `Some(record)`.

use crate::prelude::*;

// ---------- create / read ----------

#[driver_test(id(ID), scenario(crate::scenarios::document_optional_metadata))]
pub async fn create_and_read_some(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        metadata: Some(Metadata {
            author: "Alice".to_string(),
            notes: "Important".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;

    assert_struct!(created.metadata, Some(_ {
        author: "Alice",
        notes: "Important",
    }));

    let read = Document::get_by_id(&mut db, &created.id).await?;
    assert_struct!(read.metadata, Some(_ {
        author: "Alice",
        notes: "Important",
    }));

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::document_optional_metadata))]
pub async fn create_and_read_none(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    // Omitting the optional embed creates it as `None`. This exercises the
    // encoding null-guard (all columns NULL, no projection panic), the
    // INSERT...RETURNING default (the echoed record collapses to `None`), and
    // the SELECT decode sentinel.
    let created = toasty::create!(Document {
        title: "Hello".to_string(),
    })
    .exec(&mut db)
    .await?;

    assert!(created.metadata.is_none());

    let read = Document::get_by_id(&mut db, &created.id).await?;
    assert_eq!("Hello", read.title);
    assert!(read.metadata.is_none());

    Ok(())
}

// ---------- update (set whole field) ----------

#[driver_test(id(ID), scenario(crate::scenarios::document_optional_metadata))]
pub async fn update_some_to_none_and_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut doc = toasty::create!(Document {
        title: "Hello".to_string(),
        metadata: Some(Metadata {
            author: "Alice".to_string(),
            notes: "Important".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;

    // Some -> None
    doc.update().metadata(None).exec(&mut db).await?;
    assert!(
        Document::get_by_id(&mut db, &doc.id)
            .await?
            .metadata
            .is_none()
    );

    // None -> Some
    doc.update()
        .metadata(Some(Metadata {
            author: "Bob".to_string(),
            notes: "Revised".to_string(),
        }))
        .exec(&mut db)
        .await?;

    assert_struct!(Document::get_by_id(&mut db, &doc.id).await?.metadata, Some(_ {
        author: "Bob",
        notes: "Revised",
    }));

    Ok(())
}

// ---------- filtering ----------

#[driver_test(
    id(ID),
    scenario(crate::scenarios::document_optional_metadata),
    requires(scan)
)]
pub async fn filter_is_none_and_is_some(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Document {
        title: "with".to_string(),
        metadata: Some(Metadata {
            author: "Alice".to_string(),
            notes: "n".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Document {
        title: "without".to_string(),
    })
    .exec(&mut db)
    .await?;

    let none = Document::filter(Document::fields().metadata().is_none())
        .exec(&mut db)
        .await?;
    assert_eq!(1, none.len());
    assert_eq!("without", none[0].title);

    let some = Document::filter(Document::fields().metadata().is_some())
        .exec(&mut db)
        .await?;
    assert_eq!(1, some.len());
    assert_eq!("with", some[0].title);

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::document_optional_metadata),
    requires(scan)
)]
pub async fn filter_eq_whole_value(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Document {
        title: "alice".to_string(),
        metadata: Some(Metadata {
            author: "Alice".to_string(),
            notes: "n".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Document {
        title: "bob".to_string(),
        metadata: Some(Metadata {
            author: "Bob".to_string(),
            notes: "n".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Document {
        title: "without".to_string(),
    })
    .exec(&mut db)
    .await?;

    // Whole-value equality against a present embed expands to a per-column
    // comparison and excludes both the non-matching and the absent embed.
    let matched = Document::filter(Document::fields().metadata().eq(Some(Metadata {
        author: "Alice".to_string(),
        notes: "n".to_string(),
    })))
    .exec(&mut db)
    .await?;
    assert_eq!(1, matched.len());
    assert_eq!("alice", matched[0].title);

    Ok(())
}

// ---------- nested embed under Option ----------
//
// `Option<Office>` where `Office` itself embeds `Address`. Confirms the forced
// nullability and the None sentinel propagate through every flattened leaf
// column, including those of the nested embed.

#[driver_test(id(ID), scenario(crate::scenarios::company_optional_office))]
pub async fn nested_embed_under_option(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let with = toasty::create!(Company {
        name: "Acme".to_string(),
        headquarters: Some(Office {
            name: "HQ".to_string(),
            address: Address {
                street: "1 Main".to_string(),
                city: "Springfield".to_string(),
            },
        }),
    })
    .exec(&mut db)
    .await?;

    let without = toasty::create!(Company {
        name: "Remote Inc".to_string(),
    })
    .exec(&mut db)
    .await?;

    assert_struct!(Company::get_by_id(&mut db, &with.id).await?.headquarters, Some(_ {
        name: "HQ",
        address: _ { street: "1 Main", city: "Springfield" },
    }));

    assert!(
        Company::get_by_id(&mut db, &without.id)
            .await?
            .headquarters
            .is_none()
    );

    Ok(())
}
