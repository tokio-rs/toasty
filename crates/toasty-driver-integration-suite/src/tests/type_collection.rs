//! Tests for `Vec<scalar>` model fields stored as native arrays on
//! PostgreSQL. Backends without a native array column type are gated out at
//! the `#[driver_test]` level via `requires(native_array)`; the negative
//! schema-build path is covered by a dedicated `requires(not(native_array))`
//! test.

use crate::prelude::*;

/// `Vec<String>` round-trips through INSERT, RETURNING, and a fresh fetch
/// — covers both the PG bind path (driver receives `Value::List` as one
/// `text[]` parameter) and the read path (`text[]` decoded back to
/// `Value::List`).
#[driver_test(id(ID), requires(native_array))]
pub async fn vec_string_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let tags = vec!["rust".to_string(), "toasty".to_string()];
    let item = toasty::create!(Item { tags: tags.clone() })
        .exec(&mut db)
        .await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, tags);

    Ok(())
}

/// Whole-value replacement via the update builder. Verifies the PG bind
/// path on UPDATE (assignment expression rather than INSERT row).
#[driver_test(id(ID), requires(native_array))]
pub async fn vec_string_update_replace(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item {
        tags: vec!["a".to_string(), "b".to_string()],
    })
    .exec(&mut db)
    .await?;

    let new_tags = vec!["x".to_string(), "y".to_string(), "z".to_string()];
    item.update().tags(new_tags.clone()).exec(&mut db).await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, new_tags);

    Ok(())
}

/// `path.contains(value)` filter. Lowers to `value = ANY(col)` on
/// PostgreSQL — a GIN-indexable predicate when the column has the
/// appropriate index.
#[driver_test(id(ID), requires(native_array))]
pub async fn vec_string_contains_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { tags: vec!["admin".to_string(), "verified".to_string()] },
        { tags: vec!["guest".to_string()] },
        { tags: vec!["admin".to_string(), "moderator".to_string()] },
    ])
    .exec(&mut db)
    .await?;

    let admins = Item::filter(Item::fields().tags().contains("admin"))
        .exec(&mut db)
        .await?;
    assert_eq!(admins.len(), 2);

    let none = Item::filter(Item::fields().tags().contains("missing"))
        .exec(&mut db)
        .await?;
    assert_eq!(none.len(), 0);

    Ok(())
}

/// `path.is_superset([...])` (PG `@>`). Matches rows whose array contains
/// every element of the right-hand set.
#[driver_test(id(ID), requires(native_array))]
pub async fn vec_string_is_superset_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { tags: vec!["admin".to_string(), "verified".to_string()] },
        { tags: vec!["admin".to_string()] },
        { tags: vec!["verified".to_string()] },
    ])
    .exec(&mut db)
    .await?;

    let both = Item::filter(
        Item::fields()
            .tags()
            .is_superset(vec!["admin".to_string(), "verified".to_string()]),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(both.len(), 1);

    Ok(())
}

/// `path.intersects([...])` (PG `&&`). Matches rows whose array shares at
/// least one element with the right-hand set.
#[driver_test(id(ID), requires(native_array))]
pub async fn vec_string_intersects_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { tags: vec!["admin".to_string()] },
        { tags: vec!["moderator".to_string()] },
        { tags: vec!["guest".to_string()] },
    ])
    .exec(&mut db)
    .await?;

    let priv_users = Item::filter(
        Item::fields()
            .tags()
            .intersects(vec!["admin".to_string(), "moderator".to_string()]),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(priv_users.len(), 2);

    Ok(())
}

/// On backends without `native_array` (everything except PostgreSQL today),
/// a model containing a `Vec<scalar>` field is rejected at schema build with
/// a clear error message.
#[driver_test(id(ID), requires(not(native_array)))]
pub async fn vec_string_unsupported_backend(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let result = t.try_setup_db(models!(Item)).await;
    match result {
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("Vec<T>") && msg.contains("document-fields.md"),
                "expected error pointing at the design doc, got: {msg}"
            );
        }
        Ok(_) => panic!("expected schema build to reject Vec<T> field on this backend"),
    }

    Ok(())
}

/// `path.len()` and `path.is_empty()` predicates. PG `cardinality(col)`.
#[driver_test(id(ID), requires(native_array))]
pub async fn vec_string_len_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { tags: Vec::<String>::new() },
        { tags: vec!["a".to_string()] },
        { tags: vec!["a".to_string(), "b".to_string(), "c".to_string()] },
    ])
    .exec(&mut db)
    .await?;

    let empty = Item::filter(Item::fields().tags().is_empty())
        .exec(&mut db)
        .await?;
    assert_eq!(empty.len(), 1);

    let many = Item::filter(Item::fields().tags().len().gt(1))
        .exec(&mut db)
        .await?;
    assert_eq!(many.len(), 1);

    Ok(())
}
