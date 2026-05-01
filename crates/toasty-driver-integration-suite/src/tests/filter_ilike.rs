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
        { id: 2_i64, name: "ALICIA"  },
        { id: 3_i64, name: "alfred"  },
        { id: 4_i64, name: "Bob"     },
        { id: 5_i64, name: "BARRY"   },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    db
}

/// ILIKE with an uppercase pattern matches rows regardless of their case.
/// On PostgreSQL this requires `ILIKE`; on SQLite/MySQL plain `LIKE` is already
/// case-insensitive for ASCII, so the same query works.
#[driver_test(requires(sql))]
pub async fn ilike_uppercase_pattern(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(Item::fields().name().ilike("AL%".to_string()))
        .exec(&mut db)
        .await?;

    items.sort_by_key(|i| i.id);

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].name, "Alice");
    assert_eq!(items[1].name, "ALICIA");
    assert_eq!(items[2].name, "alfred");

    Ok(())
}

/// ILIKE with a lowercase pattern — symmetric to the uppercase case. Catches
/// the inverse bug if the serializer accidentally emitted a case-sensitive op.
#[driver_test(requires(sql))]
pub async fn ilike_lowercase_pattern(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(Item::fields().name().ilike("al%".to_string()))
        .exec(&mut db)
        .await?;

    items.sort_by_key(|i| i.id);

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].name, "Alice");
    assert_eq!(items[1].name, "ALICIA");
    assert_eq!(items[2].name, "alfred");

    Ok(())
}

/// ILIKE with a pattern that matches nothing — returns empty result.
#[driver_test(requires(sql))]
pub async fn ilike_no_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let items: Vec<Item> = Item::filter(Item::fields().name().ilike("ZZ%".to_string()))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}

/// ILIKE on an `Option<String>` field — matches non-null values regardless of
/// case; rows with NULL values are excluded since `NULL ILIKE pattern` is
/// unknown.
#[driver_test(requires(sql))]
pub async fn ilike_optional_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct OptItem {
        #[key]
        id: i64,
        nickname: Option<String>,
    }

    let mut db = test.setup_db(models!(OptItem)).await;

    toasty::create!(OptItem::[
        { id: 1_i64, nickname: Some("Alice".to_string())   },
        { id: 2_i64, nickname: Some("ALICIA".to_string())  },
        { id: 3_i64, nickname: Some("alfred".to_string())  },
        { id: 4_i64, nickname: Some("Bob".to_string())     },
        { id: 5_i64, nickname: None                        },
    ])
    .exec(&mut db)
    .await?;

    let mut items: Vec<OptItem> =
        OptItem::filter(OptItem::fields().nickname().ilike("al%".to_string()))
            .exec(&mut db)
            .await?;

    items.sort_by_key(|i| i.id);

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].nickname.as_deref(), Some("Alice"));
    assert_eq!(items[1].nickname.as_deref(), Some("ALICIA"));
    assert_eq!(items[2].nickname.as_deref(), Some("alfred"));

    Ok(())
}
