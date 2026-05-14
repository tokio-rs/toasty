//! Tests for `Vec<scalar>` model fields. Storage is backend-chosen
//! (`text[]` on PostgreSQL, List `L` on DynamoDB, JSON on MySQL/SQLite —
//! the JSON paths are future work). Backends without `Vec<scalar>` support
//! are gated out at the `#[driver_test]` level via `requires(vec_scalar)`;
//! the negative schema-build path is covered by a dedicated
//! `requires(not(vec_scalar))` test.

use crate::prelude::*;

/// `Vec<String>` round-trips through INSERT, RETURNING, and a fresh fetch
/// — covers both the PG bind path (driver receives `Value::List` as one
/// `text[]` parameter) and the read path (`text[]` decoded back to
/// `Value::List`).
#[driver_test(id(ID), requires(vec_scalar))]
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
#[driver_test(id(ID), requires(vec_scalar))]
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
#[driver_test(id(ID), requires(vec_scalar))]
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
#[driver_test(id(ID), requires(vec_scalar))]
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
#[driver_test(id(ID), requires(vec_scalar))]
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

/// On backends without `vec_scalar` support, a model containing a
/// `Vec<scalar>` field is rejected at schema build with a clear error
/// message.
#[driver_test(id(ID), requires(not(vec_scalar)))]
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
                msg.contains("Vec<T>") && msg.contains("does not yet support"),
                "expected schema-build rejection naming the unsupported `Vec<T>` field, got: {msg}"
            );
        }
        Ok(_) => panic!("expected schema build to reject Vec<T> field on this backend"),
    }

    Ok(())
}

/// `stmt::push(value)` appends one element atomically. Validates the
/// per-backend native append path (`||` on PG, `JSON_MERGE_PRESERVE` on
/// MySQL, `json_each + json_group_array` on SQLite, `list_append` on
/// DynamoDB).
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn vec_string_push(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::push("b"))
        .exec(&mut db)
        .await?;

    // In-memory model reflects the post-update value.
    assert_eq!(item.tags, vec!["a".to_string(), "b".to_string()]);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, vec!["a".to_string(), "b".to_string()]);

    Ok(())
}

/// `stmt::push` onto a `Vec<String>` that was created empty. Covers the
/// "initially empty" path — DynamoDB's `if_not_exists` guard and the
/// PG / JSON-backed paths over an empty array literal.
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn vec_string_push_to_empty(t: &mut Test) -> Result<(), BoxError> {
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
        tags: Vec::<String>::new(),
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::push("first"))
        .exec(&mut db)
        .await?;

    assert_eq!(item.tags, vec!["first".to_string()]);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, vec!["first".to_string()]);

    Ok(())
}

/// `stmt::extend(iter)` appends every element in order. Validates that
/// multi-element appends emit one append op per call (not one per
/// element).
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn vec_string_extend(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::extend(["b", "c", "d"]))
        .exec(&mut db)
        .await?;

    let expected = vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
    ];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::extend(empty)` is a no-op append. Exercises the path where
/// the appended list has no elements to infer an element type from —
/// `refine_update` must push the column's element type down into the
/// param so finalize doesn't see an unresolved `Ty::Unknown`.
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn vec_string_extend_empty(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::extend(Vec::<String>::new()))
        .exec(&mut db)
        .await?;

    assert_eq!(item.tags, vec!["a".to_string()]);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, vec!["a".to_string()]);

    Ok(())
}

/// `stmt::clear()` replaces the field with an empty list.
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn vec_string_clear(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::clear())
        .exec(&mut db)
        .await?;

    assert!(
        item.tags.is_empty(),
        "item.tags should be empty after clear"
    );

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert!(reloaded.tags.is_empty(), "tags should be empty after clear");

    Ok(())
}

/// `stmt::pop()` drops the trailing element. PG slicing
/// (`col[1:cardinality(col) - 1]`); other backends fall back to RMW (not
/// yet implemented).
#[driver_test(id(ID), requires(vec_pop))]
pub async fn vec_string_pop(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::pop())
        .exec(&mut db)
        .await?;

    let expected = vec!["a".to_string(), "b".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::pop()` on an already-empty collection is a no-op rather than
/// an error. Verifies the slicing expression handles the empty case
/// cleanly.
#[driver_test(id(ID), requires(vec_pop))]
pub async fn vec_string_pop_on_empty(t: &mut Test) -> Result<(), BoxError> {
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
        tags: Vec::<String>::new(),
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::pop())
        .exec(&mut db)
        .await?;

    assert!(item.tags.is_empty());

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert!(reloaded.tags.is_empty());

    Ok(())
}

/// `stmt::remove(value)` removes every matching element. PG
/// `array_remove(col, $1)`; other backends fall back to RMW (not yet
/// implemented).
#[driver_test(id(ID), requires(vec_remove))]
pub async fn vec_string_remove_value(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["admin".to_string(), "user".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::remove("admin"))
        .exec(&mut db)
        .await?;

    let expected = vec!["user".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::remove(value)` against an absent value is a no-op rather than
/// an error.
#[driver_test(id(ID), requires(vec_remove))]
pub async fn vec_string_remove_value_missing(t: &mut Test) -> Result<(), BoxError> {
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

    item.update()
        .tags(toasty::stmt::remove("missing"))
        .exec(&mut db)
        .await?;

    let expected = vec!["a".to_string(), "b".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::remove(value)` removes every element equal to the value, not
/// just the first match. Matches `array_remove` on PG and aligns with
/// the design's "remove all matching" semantic.
#[driver_test(id(ID), requires(vec_remove))]
pub async fn vec_string_remove_value_multiple_matches(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec![
            "a".to_string(),
            "dup".to_string(),
            "b".to_string(),
            "dup".to_string(),
            "c".to_string(),
        ],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::remove("dup"))
        .exec(&mut db)
        .await?;

    let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::remove_at(idx)` drops the element at the given 0-based index.
/// PG: `col[1:i] || col[i + 2:cardinality(col)]`.
#[driver_test(id(ID), requires(vec_remove_at))]
pub async fn vec_string_remove_at(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::remove_at(1usize))
        .exec(&mut db)
        .await?;

    let expected = vec!["a".to_string(), "c".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::remove_at(0)` drops the head element. Exercises the
/// boundary case where the PG prefix slice (`col[1:0]`) is empty.
#[driver_test(id(ID), requires(vec_remove_at))]
pub async fn vec_string_remove_at_head(t: &mut Test) -> Result<(), BoxError> {
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
        tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .tags(toasty::stmt::remove_at(0usize))
        .exec(&mut db)
        .await?;

    let expected = vec!["b".to_string(), "c".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `stmt::remove_at(i)` with `i >= len` is a no-op — per-row failure
/// semantics on a bulk update are rarely useful.
#[driver_test(id(ID), requires(vec_remove_at))]
pub async fn vec_string_remove_at_out_of_bounds(t: &mut Test) -> Result<(), BoxError> {
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

    item.update()
        .tags(toasty::stmt::remove_at(99usize))
        .exec(&mut db)
        .await?;

    let expected = vec!["a".to_string(), "b".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// `path.len()` and `path.is_empty()` predicates. PG `cardinality(col)`.
#[driver_test(id(ID), requires(vec_scalar))]
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
