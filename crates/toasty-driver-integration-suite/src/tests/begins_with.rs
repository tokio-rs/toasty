use crate::prelude::*;

/// Model with a composite key (partition + sort) and a non-key string attribute.
/// Used for all begins_with tests.
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

/// begins_with on the sort key. On DynamoDB this uses KeyConditionExpression;
/// on SQL it lowers to LIKE.
#[driver_test]
pub async fn begins_with_sort_key(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().begins_with("alpha".to_string())),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].sort_key, "alpha-1");
    assert_eq!(items[1].sort_key, "alpha-2");

    Ok(())
}

/// begins_with on a non-key attribute. On DynamoDB this uses FilterExpression;
/// on SQL it lowers to LIKE.
#[driver_test]
pub async fn begins_with_non_key_attr(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().name().begins_with("Al".to_string())),
    )
    .exec(&mut db)
    .await?;

    items.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Alice");
    assert_eq!(items[1].name, "Alicia");

    Ok(())
}

/// begins_with with a prefix that matches nothing — returns empty result.
#[driver_test]
pub async fn begins_with_no_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().begins_with("gamma".to_string())),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}

/// begins_with with an empty prefix — DynamoDB rejects empty string key values.
#[driver_test(requires(not(sql)))]
pub async fn begins_with_empty_prefix(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let result: toasty::Result<Vec<Item>> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().begins_with("".to_string())),
    )
    .exec(&mut db)
    .await;

    assert!(
        result.is_err(),
        "expected error when using begins_with with empty prefix on DynamoDB"
    );

    Ok(())
}

/// begins_with with an empty prefix on SQL — lowers to LIKE '%', matches all rows.
#[driver_test(requires(sql))]
pub async fn begins_with_empty_prefix_sql(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let items: Vec<Item> = Item::filter(
        Item::fields()
            .partition_id()
            .eq(1_i64)
            .and(Item::fields().sort_key().begins_with("".to_string())),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 4, "empty prefix should match all rows on SQL");

    Ok(())
}

/// begins_with on the partition key — DynamoDB returns a runtime error since
/// begins_with is not valid in a KeyConditionExpression on the partition key.
#[driver_test(requires(not(sql)))]
pub async fn begins_with_partition_key_error(test: &mut Test) -> Result<()> {
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
            .begins_with("hel".to_string()),
    )
    .exec(&mut db)
    .await;

    assert!(
        result.is_err(),
        "expected error when using begins_with on partition key"
    );

    Ok(())
}
