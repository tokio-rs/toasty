use crate::prelude::*;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: i64,
    name: String,
}

async fn setup(test: &mut Test) -> toasty::Db {
    let mut db = test.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice"   },
        { id: 2_i64, name: "Alicia"  },
        { id: 3_i64, name: "Bob"     },
        { id: 4_i64, name: "Barry"   },
        { id: 5_i64, name: "Charlie" },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    db
}

/// LIKE with a prefix pattern — returns rows where name starts with "Al".
#[driver_test(requires(sql))]
pub async fn like_basic(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(Item::fields().name().like("Al%".to_string()))
        .exec(&mut db)
        .await?;

    items.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Alice");
    assert_eq!(items[1].name, "Alicia");

    Ok(())
}

/// LIKE with a pattern that matches nothing — returns empty result.
#[driver_test(requires(sql))]
pub async fn like_no_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let items: Vec<Item> = Item::filter(Item::fields().name().like("ZZ%".to_string()))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}
