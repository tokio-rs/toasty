//! `Option<StructEmbed>` model fields.
//!
//! A nullable embedded struct stores a nullable `bool` presence column
//! (`NULL` = `None`, `true` = `Some`) plus its flattened leaf columns forced
//! nullable. `None` is `NULL` in the presence column, symmetric with
//! `Option<scalar>` and an embedded enum's discriminant. This disambiguates
//! `None` from a `Some` whose fields happen to all be empty.
//!
//! A single-column embed (a newtype `Option<Code>` where `Code(String)`) is
//! the exception: its one flattened leaf already collapses to the field name,
//! so it reuses that leaf as the head column (`NULL` = `None`) instead of a
//! dedicated presence column that would collide with it — exactly like
//! `Option<scalar>`.

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

// ---- Single-field newtype embed: `Option<Code>` over `struct Code(String)`.
//
// Regression coverage: a dedicated `bool` presence column would be named after
// the field (`code`), colliding with the newtype's flattened leaf (also `code`)
// and breaking `CREATE TABLE`. The nullable head instead reuses the single leaf
// column, so the embed maps to exactly one nullable column, like
// `Option<scalar>`. Every test here also implicitly exercises that `CREATE
// TABLE` succeeds — `setup` would fail on the duplicate column otherwise.

/// The DB schema: exactly one nullable column named after the field, with no
/// separate presence column.
#[driver_test(scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_db_schema(test: &mut Test) {
    let db = setup(test).await;
    let schema = db.schema();

    assert_struct!(schema.db.tables, [
        {
            name: =~ r"documents$",
            columns: [
                { name: "id" },
                { name: "code", nullable: true },
            ],
        },
    ]);
}

/// Round-trip both `Some` and `None`.
#[driver_test(scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_crud(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "with",
        code: Some(Code("abc".to_string())),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "without",
        code: None,
    })
    .exec(&mut db)
    .await?;

    let with = Document::get_by_id(&mut db, "with").await?;
    assert_struct!(with.code, Some(== Code("abc".to_string())));

    let without = Document::get_by_id(&mut db, "without").await?;
    assert_none!(without.code);

    Ok(())
}

/// `.is_none()` / `.is_some()` filter on the reused head (the single leaf)
/// column.
#[driver_test(requires(scan), scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_filter_presence(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "with",
        code: Some(Code("abc".to_string())),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "without",
        code: None,
    })
    .exec(&mut db)
    .await?;

    let present = Document::filter(Document::fields().code().is_some())
        .exec(&mut db)
        .await?;
    assert_struct!(present, [_ { id: "with", .. }]);

    let absent = Document::filter(Document::fields().code().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(absent, [_ { id: "without", .. }]);

    Ok(())
}

/// Driver-op coverage. `Some` writes the inner value to the single `code`
/// column; `None` writes `NULL` to it. There is no separate presence column.
#[driver_test(scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_create_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let code = column(&db, "documents", "code").index;
    test.log().clear();

    toasty::create!(Document {
        id: "a",
        code: Some(Code("xyz".to_string())),
    })
    .exec(&mut db)
    .await?;
    let row = pop_insert(test);
    assert_eq!(row[&code], Value::from("xyz"));

    toasty::create!(Document {
        id: "b",
        code: None,
    })
    .exec(&mut db)
    .await?;
    let row = pop_insert(test);
    assert_eq!(row[&code], Value::Null);

    Ok(())
}

/// Driver-op coverage for filters: `.is_none()` emits `code IS NULL` on the
/// single reused column.
#[driver_test(requires(scan), scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_filter_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let code = column(&db, "documents", "code").index;
    test.log().clear();

    let _ = Document::filter(Document::fields().code().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(pop_filter(test), Expr::IsNull({
        expr.as_expr_column_unwrap().column: == code,
    }));

    Ok(())
}

/// Updating the whole value in both directions: `Some` → `None` and
/// `None` → `Some`.
#[driver_test(scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "a",
        code: Some(Code("abc".to_string())),
    })
    .exec(&mut db)
    .await?;

    // Some -> None
    Document::filter_by_id("a")
        .update()
        .code(None)
        .exec(&mut db)
        .await?;
    assert_none!(Document::get_by_id(&mut db, "a").await?.code);

    // None -> Some
    Document::filter_by_id("a")
        .update()
        .code(Some(Code("xyz".to_string())))
        .exec(&mut db)
        .await?;
    assert_struct!(
        Document::get_by_id(&mut db, "a").await?.code,
        Some(== Code("xyz".to_string()))
    );

    Ok(())
}

/// Whole-value `.eq(Some(..))` on the reused head column matches only the equal
/// `Some` row, never `None`.
#[driver_test(requires(scan), scenario(crate::scenarios::document_optional_code))]
pub async fn option_newtype_filter_eq(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Document {
        id: "alice",
        code: Some(Code("abc".to_string())),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "bob",
        code: Some(Code("xyz".to_string())),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "without",
        code: None,
    })
    .exec(&mut db)
    .await?;

    let matches = Document::filter(Document::fields().code().eq(Some(Code("abc".to_string()))))
        .exec(&mut db)
        .await?;
    assert_struct!(matches, [_ { id: "alice", .. }]);

    Ok(())
}

/// `Option<Newtype>` where the newtype wraps an `Option`
/// (`MaybeBody(Option<String>)`) has no unambiguous single-column mapping — the
/// one column can't tell `None` from `Some(MaybeBody(None))`, and a dedicated
/// presence column would collide with the newtype leaf. It is rejected at
/// schema build rather than silently collapsing `Some(None)` to `None`.
#[driver_test]
pub async fn option_newtype_wrapping_option_rejected(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        id: String,

        body: Option<MaybeBody>,
    }

    #[derive(Debug, toasty::Embed)]
    struct MaybeBody(Option<String>);

    let err = assert_err!(test.try_setup_db(models!(Document)).await);
    let msg = err.to_string();
    assert!(
        msg.contains("`Document::body`") && msg.contains("newtype wrapping an optional value"),
        "unexpected error: {msg}"
    );
}

/// Contrast with the rejected newtype above: a *named* single field whose inner
/// is itself nullable (`Wrapper { value: Option<String> }`) keeps the dedicated
/// presence column, so it round-trips `Some(value: None)` distinctly from
/// `None` — the disambiguation a single reused column can't provide.
#[driver_test]
pub async fn option_struct_nullable_inner_disambiguates(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        id: String,

        wrapper: Option<Wrapper>,
    }

    #[derive(Debug, toasty::Embed)]
    struct Wrapper {
        value: Option<String>,
    }

    let mut db = test.setup_db(models!(Document)).await;

    // A `bool` presence column plus the nullable leaf — two columns, so `None`
    // and `Some(value: None)` are distinguishable.
    assert_struct!(db.schema().db.tables, [
        {
            name: =~ r"documents$",
            columns: [
                { name: "id" },
                { name: "wrapper", nullable: true },
                { name: "wrapper_value", nullable: true },
            ],
        },
    ]);

    toasty::create!(Document {
        id: "some_none",
        wrapper: Some(Wrapper { value: None }),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document {
        id: "none",
        wrapper: None,
    })
    .exec(&mut db)
    .await?;

    assert_struct!(
        Document::get_by_id(&mut db, "some_none").await?.wrapper,
        Some(_ { value: None })
    );
    assert_none!(Document::get_by_id(&mut db, "none").await?.wrapper);

    Ok(())
}
