use crate::prelude::*;

/// Model with a composite key (partition + sort) and a non-key string attribute.
/// Used for all starts_with tests.
#[derive(Debug, toasty::Model)]
#[key(partition = partition_id, local = sort_key)]
struct Item {
    partition_id: i64,
    sort_key: String,
    name: String,
}

async fn setup(test: &mut Test) -> toasty::Db {
    let mut db = test.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { partition_id: 1_i64, sort_key: "alpha-1", name: "Alice" },
        { partition_id: 1_i64, sort_key: "alpha-2", name: "Alicia" },
        { partition_id: 1_i64, sort_key: "beta-1",  name: "Bob"   },
        { partition_id: 1_i64, sort_key: "beta-2",  name: "Barry" },
        { partition_id: 2_i64, sort_key: "alpha-1", name: "Carol" },
    ])
    .exec(&mut db)
    .await
    .unwrap();

    db
}

/// starts_with on the sort key. On DynamoDB this uses KeyConditionExpression;
/// on SQL it lowers to LIKE.
#[driver_test]
pub async fn starts_with_sort_key(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().starts_with("alpha".to_string())),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].sort_key, "alpha-1");
    assert_eq!(items[1].sort_key, "alpha-2");

    Ok(())
}

/// starts_with on a non-key attribute. On DynamoDB this uses FilterExpression;
/// on SQL it lowers to LIKE.
#[driver_test]
pub async fn starts_with_non_key_attr(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().name().starts_with("Al".to_string())),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Alice");
    assert_eq!(items[1].name, "Alicia");

    Ok(())
}

/// starts_with with a prefix that matches nothing — returns empty result.
#[driver_test]
pub async fn starts_with_no_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().starts_with("gamma".to_string())),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}

/// starts_with with an empty prefix — DynamoDB rejects empty string key values.
#[driver_test(requires(not(sql)))]
pub async fn starts_with_empty_prefix(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let result: toasty::Result<Vec<Item>> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().starts_with("".to_string())),
    )
    .exec(&mut db)
    .await;

    assert!(
        result.is_err(),
        "expected error when using starts_with with empty prefix on DynamoDB"
    );

    Ok(())
}

/// starts_with with an empty prefix on SQL — lowers to LIKE '%', matches all rows.
#[driver_test(requires(sql))]
pub async fn starts_with_empty_prefix_sql(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().starts_with("".to_string())),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 4, "empty prefix should match all rows on SQL");

    Ok(())
}

/// starts_with on an `Option<String>` field — matches non-null values with
/// the given prefix; rows with NULL values are excluded.
#[driver_test]
pub async fn starts_with_optional_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = partition_id, local = id)]
    struct OptItem {
        partition_id: i64,
        id: i64,
        nickname: Option<String>,
    }

    let mut db = test.setup_db(models!(OptItem)).await;

    toasty::create!(OptItem::[
        { partition_id: 1_i64, id: 1_i64, nickname: Some("Ali".to_string())     },
        { partition_id: 1_i64, id: 2_i64, nickname: Some("Alicia".to_string())  },
        { partition_id: 1_i64, id: 3_i64, nickname: Some("Bob".to_string())     },
        { partition_id: 1_i64, id: 4_i64, nickname: None                        },
    ])
    .exec(&mut db)
    .await?;

    let mut items: Vec<OptItem> = OptItem::filter(
        OptItem::fields()
            .partition_id()
            .eq(1_i64)
            .and(OptItem::fields().nickname().starts_with("Al".to_string())),
    )
    .exec(&mut db)
    .await?;

    items.sort_by_key(|i| i.id);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].nickname.as_deref(), Some("Ali"));
    assert_eq!(items[1].nickname.as_deref(), Some("Alicia"));

    Ok(())
}

/// starts_with on the partition key — DynamoDB returns a runtime error since
/// starts_with is not valid in a KeyConditionExpression on the partition key.
#[driver_test(requires(not(sql)))]
pub async fn starts_with_partition_key_error(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = partition_id, local = sort_key)]
    struct StringKeyItem {
        partition_id: String,
        sort_key: String,
    }

    let mut db = test.setup_db(models!(StringKeyItem)).await;

    StringKeyItem::create()
        .partition_id("hello")
        .sort_key("world")
        .exec(&mut db)
        .await?;

    let result = StringKeyItem::filter(
        StringKeyItem::fields()
            .partition_id()
            .starts_with("hel".to_string()),
    )
    .exec(&mut db)
    .await;

    assert!(
        result.is_err(),
        "expected error when using starts_with on partition key"
    );

    Ok(())
}
