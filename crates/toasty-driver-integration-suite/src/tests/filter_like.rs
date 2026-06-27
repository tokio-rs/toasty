use crate::prelude::*;

/// LIKE with a prefix pattern — returns rows where name starts with "Al".
#[driver_test(requires(sql), scenario(crate::scenarios::fixed_item_name))]
pub async fn like_basic(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

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
#[driver_test(requires(sql), scenario(crate::scenarios::fixed_item_name))]
pub async fn like_no_match(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

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

    let items: Vec<Item> = Item::filter(Item::fields().name().like("ZZ%".to_string()))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}

/// LIKE on an `Option<String>` field — matches non-null values; rows with NULL
/// values are excluded since `NULL LIKE pattern` is unknown.
#[driver_test(requires(sql))]
pub async fn like_optional_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct OptItem {
        #[key]
        id: i64,
        nickname: Option<String>,
    }

    let mut db = test.setup_db(models!(OptItem)).await;

    toasty::create!(OptItem::[
        { id: 1_i64, nickname: Some("Alice".to_string())   },
        { id: 2_i64, nickname: Some("Alicia".to_string())  },
        { id: 3_i64, nickname: Some("Bob".to_string())     },
        { id: 4_i64, nickname: None                        },
    ])
    .exec(&mut db)
    .await?;

    let mut items: Vec<OptItem> =
        OptItem::filter(OptItem::fields().nickname().like("Al%".to_string()))
            .exec(&mut db)
            .await?;

    items.sort_by_key(|i| i.id);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].nickname.as_deref(), Some("Alice"));
    assert_eq!(items[1].nickname.as_deref(), Some("Alicia"));

    Ok(())
}

#[driver_test(requires(sql), scenario(crate::scenarios::fixed_item_name))]
pub async fn like_escape_percent(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice%1" },
        { id: 2_i64, name: "AliceA1" },
        { id: 3_i64, name: "Alice"  },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .name()
            .like_with_escape("Alice\\%%".to_string(), '\\'),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Alice%1");

    Ok(())
}

#[driver_test(requires(sql), scenario(crate::scenarios::fixed_item_name))]
pub async fn like_escape_underscore(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "foo_bar" },
        { id: 2_i64, name: "fooXbar" },
        { id: 3_i64, name: "foobar"  },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .name()
            .like_with_escape("foo\\_bar".to_string(), '\\'),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "foo_bar");

    Ok(())
}

#[driver_test(requires(sql), scenario(crate::scenarios::fixed_item_name))]
pub async fn like_escape_keeps_wildcards(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { id: 1_i64, name: "Alice%1" },
        { id: 2_i64, name: "Alice%2" },
        { id: 3_i64, name: "AliceA1" },
        { id: 4_i64, name: "Bob"     },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .name()
            .like_with_escape("Alice\\%%".to_string(), '\\'),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Alice%1");
    assert_eq!(items[1].name, "Alice%2");

    Ok(())
}
