//! Tests for `Vec<scalar>` model fields. Storage is backend-chosen
//! (`text[]` on PostgreSQL, List `L` on DynamoDB, JSON on MySQL/SQLite —
//! the JSON paths are future work). Backends without `Vec<scalar>` support
//! are gated out at the `#[driver_test]` level via `requires(vec_scalar)`;
//! the negative schema-build path is covered by a dedicated
//! `requires(not(vec_scalar))` test.

use crate::prelude::*;

#[driver_test(requires(and(native_array, native_timestamp)))]
pub async fn vec_timestamp_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        values: Vec<jiff::Timestamp>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let values = vec![
        "2023-11-14T22:13:20.123456Z".parse()?,
        "2020-01-02T03:04:05.654321Z".parse()?,
    ];

    let item = toasty::create!(Item {
        values: values.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.values, values);

    Ok(())
}

#[driver_test(requires(vec_scalar))]
pub async fn vec_zoned_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        values: Vec<jiff::Zoned>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let values = vec![
        "2021-06-15T14:30:00-04:00[America/New_York]".parse()?,
        "2025-12-31T23:59:59+09:00[Asia/Tokyo]".parse()?,
    ];

    let item = toasty::create!(Item {
        values: values.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.values, values);

    Ok(())
}

#[driver_test(requires(and(native_array, native_date)))]
pub async fn vec_date_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        values: Vec<jiff::civil::Date>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let values = vec![
        jiff::civil::date(2025, 6, 15),
        jiff::civil::date(2020, 1, 2),
    ];

    let item = toasty::create!(Item {
        values: values.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.values, values);

    Ok(())
}

#[driver_test(requires(and(native_array, native_time)))]
pub async fn vec_time_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        values: Vec<jiff::civil::Time>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let values = vec![
        jiff::civil::time(9, 30, 45, 123_456_000),
        jiff::civil::time(3, 4, 5, 654_321_000),
    ];

    let item = toasty::create!(Item {
        values: values.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.values, values);

    Ok(())
}

#[driver_test(requires(and(native_array, native_datetime)))]
pub async fn vec_datetime_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        values: Vec<jiff::civil::DateTime>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let values = vec![
        jiff::civil::datetime(2025, 6, 15, 9, 30, 45, 123_456_000),
        jiff::civil::datetime(2020, 1, 2, 3, 4, 5, 654_321_000),
    ];

    let item = toasty::create!(Item {
        values: values.clone(),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.values, values);

    Ok(())
}

/// `Vec<String>` round-trips through INSERT, RETURNING, and a fresh fetch
/// — covers both the PG bind path (driver receives `Value::List` as one
/// `text[]` parameter) and the read path (`text[]` decoded back to
/// `Value::List`).
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_create_get(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_update_replace(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_contains_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_is_superset_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_intersects_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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
#[driver_test(requires(not(vec_scalar)))]
pub async fn vec_string_unsupported_backend(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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

/// Basic `Vec<scalar>` mutations supported by every vector-capable backend.
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_basic_mutations(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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

    // Apply folds same-projection operations into one equivalent append.
    let mut item = toasty::create!(Item {
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;
    item.update()
        .tags(toasty::stmt::apply([
            toasty::stmt::push("b"),
            toasty::stmt::push("c"),
        ]))
        .exec(&mut db)
        .await?;
    let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    assert_eq!(item.tags, expected);
    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    // An initially empty collection exercises backend empty-list guards.
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

    // Extend appends all elements in order as one operation.
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

    // Empty extend must still infer the collection element type.
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

/// `stmt::apply([])` on a `Vec<scalar>` is a no-op: the surface API's
/// empty Apply loop adds no entry to the assignments map. Run alongside
/// a non-`Vec<scalar>` field so the engine verifier doesn't reject the
/// otherwise-empty update.
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_apply_empty_is_noop(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item {
        name: "n",
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;

    item.update()
        .name("n2")
        .tags(toasty::stmt::apply::<toasty::stmt::List<String>>([]))
        .exec(&mut db)
        .await?;

    assert_eq!(item.name, "n2");
    assert_eq!(item.tags, vec!["a".to_string()]);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, vec!["a".to_string()]);

    Ok(())
}

/// `stmt::pop()` for populated and empty collections.
#[driver_test(requires(vec_pop))]
pub async fn vec_string_pop_cases(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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

    // Popping an already-empty collection is a no-op.
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

/// `stmt::remove(value)` for one, no, and multiple matches.
#[driver_test(requires(vec_remove))]
pub async fn vec_string_remove_value_cases(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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

    // Removing an absent value is a no-op.
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

    // Every matching element is removed, not only the first.
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

/// `stmt::remove_at(idx)` for middle, head, and out-of-bounds indexes.
#[driver_test(requires(vec_remove_at))]
pub async fn vec_string_remove_at_cases(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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

    // Removing the head exercises an empty prefix slice.
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

    // Out-of-bounds removal is a no-op.
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
#[driver_test(requires(vec_scalar))]
pub async fn vec_string_len_filter(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
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
