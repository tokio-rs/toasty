use crate::prelude::*;

/// `.select(field)` on a `Query<List<Item>>` returns a `Query<List<String>>`
/// whose `.exec()` produces a `Vec<String>` of the projected column.
#[driver_test(scenario(crate::scenarios::fixed_item_name_quantity))]
pub async fn select_single_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let mut names: Vec<String> = Item::all()
        .select(Item::fields().name())
        .exec(&mut db)
        .await?;

    names.sort();

    assert_eq!(
        names,
        vec![
            "Alice".to_string(),
            "Bob".to_string(),
            "Charlie".to_string()
        ]
    );

    Ok(())
}

/// `.select((f1, f2))` returns a `Query<List<(T1, T2)>>` whose `.exec()`
/// produces a `Vec` of tuples.
#[driver_test(scenario(crate::scenarios::fixed_item_name_quantity))]
pub async fn select_tuple(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let mut pairs: Vec<(i64, String)> = Item::all()
        .select((Item::fields().id(), Item::fields().name()))
        .exec(&mut db)
        .await?;

    pairs.sort_by_key(|(id, _)| *id);

    assert_eq!(
        pairs,
        vec![
            (1_i64, "Alice".to_string()),
            (2_i64, "Bob".to_string()),
            (3_i64, "Charlie".to_string()),
        ]
    );

    Ok(())
}

/// `.select(...)` composes with `.filter(...)`: the projection sees only rows
/// matching the filter expression.
#[driver_test(scenario(crate::scenarios::fixed_item_name_quantity))]
pub async fn select_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let mut names: Vec<String> = Item::filter(Item::fields().quantity().gt(5_i64))
        .select(Item::fields().name())
        .exec(&mut db)
        .await?;

    names.sort();

    assert_eq!(names, vec!["Alice".to_string(), "Charlie".to_string()]);

    Ok(())
}

/// `.select(...).first()` lifts the outer container to `Option<T>`.
#[driver_test(scenario(crate::scenarios::fixed_item_name_quantity))]
pub async fn select_first(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let name: Option<String> = Item::filter(Item::fields().id().eq(2_i64))
        .select(Item::fields().name())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(name.as_deref(), Some("Bob"));

    Ok(())
}

/// A fully constant projection is planned as a `Repeat` over the data load.
/// The repeated output must forward the data load's page cursors so the page
/// can still advance.
#[driver_test(scenario(crate::scenarios::fixed_item_name_quantity), requires(sql))]
pub async fn select_const_preserves_pagination(test: &mut Test) -> Result<()> {
    use toasty::stmt::{Page, Paginate};

    let mut db = setup(test).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let query = Item::all()
        .order_by(Item::fields().id().asc())
        .select::<i64, i64>(1_i64);
    let first: Page<i64> = Paginate::new(query, 2).exec(&mut db).await?;

    assert_eq!(first.items, [1, 1]);
    assert!(first.has_next());

    let second = first.next(&mut db).await?.unwrap();
    assert_eq!(second.items, [1]);
    assert!(!second.has_next());

    Ok(())
}

/// `.select(...).first()` returns `None` when no rows match.
#[driver_test(scenario(crate::scenarios::fixed_item_name_quantity))]
pub async fn select_first_no_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let name: Option<String> = Item::filter(Item::fields().id().eq(999_i64))
        .select(Item::fields().name())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(name, None);

    Ok(())
}
