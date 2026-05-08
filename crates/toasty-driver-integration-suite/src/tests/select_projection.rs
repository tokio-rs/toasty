use crate::prelude::*;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: i64,
    name: String,
    quantity: i64,
}

async fn setup(test: &mut Test) -> toasty::Db {
    let mut db = test.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice",   quantity: 7_i64  },
        { id: 2_i64, name: "Bob",     quantity: 3_i64  },
        { id: 3_i64, name: "Charlie", quantity: 11_i64 },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    db
}

/// `.select(field)` on a `Query<List<Item>>` returns a `Query<List<String>>`
/// whose `.exec()` produces a `Vec<String>` of the projected column.
#[driver_test]
pub async fn select_single_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

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
#[driver_test]
pub async fn select_tuple(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

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
#[driver_test]
pub async fn select_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut names: Vec<String> = Item::filter(Item::fields().quantity().gt(5_i64))
        .select(Item::fields().name())
        .exec(&mut db)
        .await?;

    names.sort();

    assert_eq!(names, vec!["Alice".to_string(), "Charlie".to_string()]);

    Ok(())
}

/// `.select(...).first()` lifts the outer container to `Option<T>`.
#[driver_test]
pub async fn select_first(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let name: Option<String> = Item::filter(Item::fields().id().eq(2_i64))
        .select(Item::fields().name())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(name.as_deref(), Some("Bob"));

    Ok(())
}

/// `.select(...).first()` returns `None` when no rows match.
#[driver_test]
pub async fn select_first_no_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let name: Option<String> = Item::filter(Item::fields().id().eq(999_i64))
        .select(Item::fields().name())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(name, None);

    Ok(())
}
