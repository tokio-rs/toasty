use crate::prelude::*;

/// Filters on a numeric field using `between()`, returning only rows in range.
#[driver_test(id(ID))]
pub async fn filter_between_numeric(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        score: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for score in [10_i64, 20, 30, 40, 50] {
        Item::create().score(score).exec(&mut db).await?;
    }

    let mut items: Vec<_> = Item::filter(Item::fields().score().between(20_i64, 40_i64))
        .exec(&mut db)
        .await?;

    items.sort_by_key(|i| i.score);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].score, 20);
    assert_eq!(items[1].score, 30);
    assert_eq!(items[2].score, 40);

    Ok(())
}

/// Verifies that `between()` bounds are inclusive on both ends.
#[driver_test(id(ID))]
pub async fn filter_between_inclusive_bounds(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        value: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for v in [1_i64, 5, 10] {
        Item::create().value(v).exec(&mut db).await?;
    }

    // Both boundary values (1 and 10) should be included.
    let items: Vec<_> = Item::filter(Item::fields().value().between(1_i64, 10_i64))
        .exec(&mut db)
        .await?;
    assert_eq!(items.len(), 3);

    // Exact lower bound.
    let items: Vec<_> = Item::filter(Item::fields().value().between(1_i64, 1_i64))
        .exec(&mut db)
        .await?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].value, 1);

    // Exact upper bound.
    let items: Vec<_> = Item::filter(Item::fields().value().between(10_i64, 10_i64))
        .exec(&mut db)
        .await?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].value, 10);

    Ok(())
}

/// Verifies that `between()` returns nothing when the range excludes all rows.
#[driver_test(id(ID))]
pub async fn filter_between_empty_result(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        value: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for v in [1_i64, 2, 3] {
        Item::create().value(v).exec(&mut db).await?;
    }

    let items: Vec<_> = Item::filter(Item::fields().value().between(100_i64, 200_i64))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}

/// Filters on a string field using `between()`.
#[driver_test(id(ID))]
pub async fn filter_between_string(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for name in ["apple", "banana", "cherry", "date", "elderberry"] {
        Item::create().name(name).exec(&mut db).await?;
    }

    let items: Vec<_> = Item::filter(Item::fields().name().between("banana", "date"))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 3);
    assert_eq_unordered!(
        items.iter().map(|i| i.name.as_str()),
        ["banana", "cherry", "date"]
    );

    Ok(())
}

/// For DynamoDB: uses `between()` as a key condition on the sort key.
#[driver_test(requires(not(sql)))]
pub async fn filter_between_sort_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(pk, sk)]
    struct Item {
        pk: i64,
        sk: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for sk in 0..10_i64 {
        Item::create().pk(1).sk(sk).exec(&mut db).await?;
    }

    // between on the sort key (key condition expression path in DynamoDB)
    let mut items: Vec<_> = Item::filter(
        Item::fields()
            .pk()
            .eq(1_i64)
            .and(Item::fields().sk().between(3_i64, 6_i64)),
    )
    .exec(&mut db)
    .await?;

    items.sort_by_key(|i| i.sk);
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].sk, 3);
    assert_eq!(items[1].sk, 4);
    assert_eq!(items[2].sk, 5);
    assert_eq!(items[3].sk, 6);

    Ok(())
}

/// For DynamoDB: uses `between()` as a key condition on a GSI sort key.
#[driver_test(requires(not(sql)))]
pub async fn filter_between_gsi_sort_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(partition = [player_id], local = [score])]
    struct Match {
        id: String,
        player_id: String,
        score: i64,
    }

    let mut db = test.setup_db(models!(Match)).await;

    toasty::create!(Match::[
        { id: "m1", player_id: "p1", score: 10_i64 },
        { id: "m2", player_id: "p1", score: 20_i64 },
        { id: "m3", player_id: "p1", score: 30_i64 },
        { id: "m4", player_id: "p1", score: 40_i64 },
        { id: "m5", player_id: "p1", score: 50_i64 },
        { id: "m6", player_id: "p2", score: 25_i64 },
    ])
    .exec(&mut db)
    .await?;

    let mut matches: Vec<Match> = Match::filter(
        Match::fields()
            .player_id()
            .eq("p1")
            .and(Match::fields().score().between(20_i64, 40_i64)),
    )
    .exec(&mut db)
    .await?;

    matches.sort_by_key(|m| m.score);
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].score, 20);
    assert_eq!(matches[1].score, 30);
    assert_eq!(matches[2].score, 40);

    Ok(())
}
