//! `Option<StructEmbed>` model fields.
//!
//! A nullable embedded struct stores a nullable `bool` presence column
//! (`NULL` = `None`, `true` = `Some`) plus its flattened leaf columns forced
//! nullable. `None` is `NULL` in the presence column, symmetric with
//! `Option<scalar>` and an embedded enum's discriminant. This disambiguates
//! `None` from a `Some` whose fields happen to all be empty.

use crate::helpers::column;
use crate::prelude::*;

use toasty_core::stmt::{Expr, Value};

/// The DB schema: a nullable `bool` presence column named after the field,
/// plus one nullable column per flattened leaf field.
#[driver_test(scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_db_schema(test: &mut Test) {
    let db = setup(test).await;
    let schema = db.schema();

    assert_struct!(schema.db.tables, [
        {
            name: =~ r"documents$",
            columns: [
                { name: "id" },
                { name: "metadata", nullable: true },
                { name: "metadata_author", nullable: true },
                { name: "metadata_note", nullable: true },
            ],
        },
    ]);
}

/// Round-trip both `Some` and `None`.
#[driver_test(scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_crud(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "with",
        metadata: Some(Metadata {
            author: "alice".to_string(),
            note: "hi".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Document {
        id: "without",
        metadata: None,
    })
    .exec(&mut db)
    .await?;

    let with = Document::get_by_id(&mut db, "with").await?;
    assert_struct!(with.metadata, Some(_ {
        author: "alice",
        note: "hi",
    }));

    let without = Document::get_by_id(&mut db, "without").await?;
    assert_none!(without.metadata);

    Ok(())
}

/// `.is_none()` / `.is_some()` filter on the presence column.
#[driver_test(requires(scan), scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_filter_presence(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "with",
        metadata: Some(Metadata {
            author: "alice".to_string(),
            note: "hi".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "without",
        metadata: None,
    })
    .exec(&mut db)
    .await?;

    let present = Document::filter(Document::fields().metadata().is_some())
        .exec(&mut db)
        .await?;
    assert_struct!(present, [_ { id: "with", .. }]);

    let absent = Document::filter(Document::fields().metadata().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(absent, [_ { id: "without", .. }]);

    Ok(())
}

/// Whole-value `.eq(Some(..))` matches only the equal `Some` row, never `None`.
#[driver_test(requires(scan), scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_filter_eq(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "alice",
        metadata: Some(Metadata {
            author: "alice".to_string(),
            note: "hi".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "bob",
        metadata: Some(Metadata {
            author: "bob".to_string(),
            note: "yo".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "without",
        metadata: None,
    })
    .exec(&mut db)
    .await?;

    let matches = Document::filter(Document::fields().metadata().eq(Some(Metadata {
        author: "alice".to_string(),
        note: "hi".to_string(),
    })))
    .exec(&mut db)
    .await?;
    assert_struct!(matches, [_ { id: "alice", .. }]);

    Ok(())
}

/// Updating the whole value in both directions: `Some` → `None` and `None` → `Some`.
#[driver_test(scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "a",
        metadata: Some(Metadata {
            author: "alice".to_string(),
            note: "hi".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "b",
        metadata: None,
    })
    .exec(&mut db)
    .await?;

    // Some -> None
    Document::filter_by_id("a")
        .update()
        .metadata(None)
        .exec(&mut db)
        .await?;
    assert_none!(Document::get_by_id(&mut db, "a").await?.metadata);

    // None -> Some
    Document::filter_by_id("b")
        .update()
        .metadata(Some(Metadata {
            author: "bob".to_string(),
            note: "yo".to_string(),
        }))
        .exec(&mut db)
        .await?;
    assert_struct!(Document::get_by_id(&mut db, "b").await?.metadata, Some(_ {
        author: "bob",
        note: "yo",
    }));

    Ok(())
}

/// Driver-op coverage for create. A `Some` writes `true` to the presence column
/// plus the leaf values; a `None` writes `NULL` to the presence column *and*
/// every leaf column. The column set is identical in both cases — `None` is a
/// concrete all-`NULL` row, not an omitted/absent one.
#[driver_test(scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_create_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let presence = column(&db, "documents", "metadata").index;
    let author = column(&db, "documents", "metadata_author").index;
    let note = column(&db, "documents", "metadata_note").index;
    test.log().clear();

    toasty::create!(Document {
        id: "a",
        metadata: Some(Metadata {
            author: "alice".to_string(),
            note: "hi".to_string(),
        }),
    })
    .exec(&mut db)
    .await?;
    let row = pop_insert(test);
    assert_eq!(row[&presence], Value::from(true));
    assert_eq!(row[&author], Value::from("alice"));
    assert_eq!(row[&note], Value::from("hi"));

    toasty::create!(Document {
        id: "b",
        metadata: None,
    })
    .exec(&mut db)
    .await?;
    let row = pop_insert(test);
    assert_eq!(row[&presence], Value::Null);
    assert_eq!(row[&author], Value::Null);
    assert_eq!(row[&note], Value::Null);

    Ok(())
}

/// Driver-op coverage for filters. `.is_none()` / `.is_some()` emit a predicate
/// on the single presence column (`IS NULL` / `NOT (.. IS NULL)`), never a
/// check distributed across the leaf columns.
#[driver_test(requires(scan), scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_filter_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let presence = column(&db, "documents", "metadata").index;
    test.log().clear();

    // `.is_none()` → `metadata IS NULL`.
    let _ = Document::filter(Document::fields().metadata().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(pop_filter(test), Expr::IsNull({
        expr.as_expr_column_unwrap().column: == presence,
    }));

    // `.is_some()` → `NOT (metadata IS NULL)`.
    let _ = Document::filter(Document::fields().metadata().is_some())
        .exec(&mut db)
        .await?;
    let Expr::Not(not) = pop_filter(test) else {
        panic!("expected NOT");
    };
    assert_struct!(*not.expr, Expr::IsNull({
        expr.as_expr_column_unwrap().column: == presence,
    }));

    Ok(())
}

/// Driver-op coverage for updates. Setting `Some` assigns `true` + the leaf
/// values; setting `None` assigns `NULL` to the presence column and every leaf.
#[driver_test(scenario(crate::scenarios::document_optional_metadata))]
pub async fn option_embed_update_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let presence = column(&db, "documents", "metadata").index;
    let author = column(&db, "documents", "metadata_author").index;
    let note = column(&db, "documents", "metadata_note").index;

    toasty::create!(Document {
        id: "a",
        metadata: None,
    })
    .exec(&mut db)
    .await?;

    // None -> Some.
    test.log().clear();
    Document::filter_by_id("a")
        .update()
        .metadata(Some(Metadata {
            author: "alice".to_string(),
            note: "hi".to_string(),
        }))
        .exec(&mut db)
        .await?;
    let set = pop_update(test);
    assert_eq!(set[&presence], Value::from(true));
    assert_eq!(set[&author], Value::from("alice"));
    assert_eq!(set[&note], Value::from("hi"));

    // Some -> None.
    test.log().clear();
    Document::filter_by_id("a")
        .update()
        .metadata(None)
        .exec(&mut db)
        .await?;
    let set = pop_update(test);
    assert_eq!(set[&presence], Value::Null);
    assert_eq!(set[&author], Value::Null);
    assert_eq!(set[&note], Value::Null);

    Ok(())
}
