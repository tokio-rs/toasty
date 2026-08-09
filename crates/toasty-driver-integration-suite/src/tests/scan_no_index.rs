use crate::prelude::*;
use toasty::stmt::Page;

/// Scan with no filter predicate returns all rows.
#[driver_test]
pub async fn scan_no_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { name: "Alice" },
        { name: "Bob" },
        { name: "Charlie" },
    ])
    .exec(&mut db)
    .await?;

    let mut results: Vec<Item> = Item::all().exec(&mut db).await?;
    results.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(3, results.len());
    assert_eq!("Alice", results[0].name);
    assert_eq!("Bob", results[1].name);
    assert_eq!("Charlie", results[2].name);

    Ok(())
}

/// DELETE filtered on a non-indexed field. On the scan path the engine has to
/// collect the primary keys of the matching rows before it can delete them.
#[driver_test(id(ID))]
pub async fn scan_delete_by_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        category: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { category: "keep" },
        { category: "drop" },
        { category: "drop" },
    ])
    .exec(&mut db)
    .await?;

    Item::filter(Item::fields().category().eq("drop"))
        .delete()
        .exec(&mut db)
        .await?;

    let remaining: Vec<Item> = Item::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("keep", remaining[0].category);

    Ok(())
}

/// UPDATE filtered on a non-indexed field, the mutation counterpart to
/// [`scan_delete_by_filter`].
#[driver_test(id(ID))]
pub async fn scan_update_by_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        category: String,
        label: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { category: "keep", label: "old" },
        { category: "stale", label: "old" },
        { category: "stale", label: "old" },
    ])
    .exec(&mut db)
    .await?;

    Item::filter(Item::fields().category().eq("stale"))
        .update()
        .label("new")
        .exec(&mut db)
        .await?;

    let mut results: Vec<Item> = Item::all().exec(&mut db).await?;
    results.sort_by(|a, b| a.category.cmp(&b.category));

    assert_eq!(3, results.len());
    assert_eq!("old", results[0].label);
    assert_eq!("new", results[1].label);
    assert_eq!("new", results[2].label);

    Ok(())
}

/// UPDATE on the scan path that moves a unique column onto a value already
/// owned by another row must surface the constraint failure — not classify it
/// as a filter miss and report zero rows updated.
#[driver_test]
pub async fn scan_update_unique_conflict_is_error(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        email: String,
        category: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { email: "taken@example.com", category: "keep" },
        { email: "free@example.com", category: "move" },
    ])
    .exec(&mut db)
    .await?;

    let res = Item::filter(Item::fields().category().eq("move"))
        .update()
        .email("taken@example.com")
        .exec(&mut db)
        .await;

    assert!(
        res.is_err(),
        "expected unique-constraint failure, got {res:?}"
    );

    // The row targeted by the update keeps its original email.
    let item = Item::get_by_email(&mut db, "free@example.com").await?;
    assert_eq!("move", item.category);

    Ok(())
}

/// DELETE on the scan path for a model with a composite primary key — every
/// key column has to be read back from the scan.
#[driver_test]
pub async fn scan_delete_by_filter_composite_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,
        name: String,
        position: String,
    }

    let mut db = t.setup_db(models!(Player)).await;

    toasty::create!(Player::[
        { team: "red", name: "alice", position: "keeper" },
        { team: "red", name: "bob", position: "striker" },
        { team: "blue", name: "carol", position: "striker" },
    ])
    .exec(&mut db)
    .await?;

    Player::filter(Player::fields().position().eq("striker"))
        .delete()
        .exec(&mut db)
        .await?;

    let remaining: Vec<Player> = Player::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("alice", remaining[0].name);

    Ok(())
}

/// Scan with an OR filter on non-indexed fields.
#[driver_test]
pub async fn scan_or_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { name: "Alice" },
        { name: "Bob" },
        { name: "Charlie" },
    ])
    .exec(&mut db)
    .await?;

    let mut results: Vec<Item> = Item::filter(
        Item::fields()
            .name()
            .eq("Alice")
            .or(Item::fields().name().eq("Charlie")),
    )
    .exec(&mut db)
    .await?;
    results.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(2, results.len());
    assert_eq!("Alice", results[0].name);
    assert_eq!("Charlie", results[1].name);

    Ok(())
}

/// Scan respects a limit — at most `limit` rows are returned.
#[driver_test]
pub async fn scan_with_limit(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { name: "Alice" },
        { name: "Bob" },
        { name: "Charlie" },
        { name: "Dave" },
        { name: "Eve" },
    ])
    .exec(&mut db)
    .await?;

    let results: Vec<Item> = Item::all().limit(3).exec(&mut db).await?;

    assert!(
        results.len() <= 3,
        "expected at most 3 results, got {}",
        results.len()
    );

    Ok(())
}

/// Scan with a filter AND a limit returns exactly `limit` matching rows even
/// when the table contains more non-matching rows than `limit`. This exercises
/// the loop-with-ExclusiveStartKey path in the DynamoDB driver.
#[driver_test(requires(not(sql)))]
pub async fn scan_limit_with_filter_returns_correct_count(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        category: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Insert 20 rows: 10 "match" and 10 "other". With limit(5) and DynamoDB's
    // pre-filter Limit semantics, a naive single-call implementation would
    // frequently return fewer than 5 rows when the examined items are mostly
    // "other".
    for _i in 0..10_i64 {
        toasty::create!(Item { category: "match" })
            .exec(&mut db)
            .await?;
        toasty::create!(Item { category: "other" })
            .exec(&mut db)
            .await?;
    }

    let results: Vec<Item> = Item::filter(Item::fields().category().eq("match"))
        .limit(5)
        .exec(&mut db)
        .await?;

    assert_eq!(
        5,
        results.len(),
        "expected exactly 5 matching rows, got {}",
        results.len()
    );
    assert!(
        results.iter().all(|r| r.category == "match"),
        "all returned rows should have category 'match'"
    );

    Ok(())
}

/// Cursor-based pagination over a full-table scan returns all rows across
/// multiple pages with no duplicates.
#[driver_test(requires(not(sql)))]
pub async fn scan_paginate_multi_page(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        score: i64,
    }

    let mut db = t.setup_db(models!(Item)).await;

    for score in 1_i64..=15 {
        toasty::create!(Item { score }).exec(&mut db).await?;
    }

    // Paginate with page_size=5 — should yield 3 pages of 5 rows each.
    let mut page: Page<Item> = Item::all().paginate(5).exec(&mut db).await?;

    let mut all_items: Vec<Item> = std::mem::take(&mut page.items);
    while let Some(mut next) = page.next(&mut db).await? {
        all_items.append(&mut next.items);
        page = next;
    }

    assert_eq!(
        15,
        all_items.len(),
        "expected 15 total rows across all pages"
    );

    // No duplicate IDs.
    let mut scores: Vec<i64> = all_items.iter().map(|r| r.score).collect();
    scores.sort_unstable();
    scores.dedup();
    assert_eq!(15, scores.len(), "expected 15 unique scores");
    assert_eq!((1..=15).collect::<Vec<_>>(), scores);

    Ok(())
}

/// ORDER BY on a scan-path query is an error on DynamoDB — the Scan API
/// returns items in an unspecified order so sorted results cannot be
/// guaranteed.
#[driver_test(requires(not(sql)))]
pub async fn scan_order_by_is_error(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        score: i64,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { score: 10_i64 },
        { score: 30_i64 },
        { score: 20_i64 },
        { score: 50_i64 },
        { score: 40_i64 },
    ])
    .exec(&mut db)
    .await?;

    let result: toasty::Result<Vec<Item>> = Item::all()
        .order_by(Item::fields().score().desc())
        .exec(&mut db)
        .await;

    assert!(
        result.is_err(),
        "expected error when using ORDER BY on a scan-path query on DynamoDB"
    );

    Ok(())
}

/// ORDER BY on a full-table scan works on SQL drivers — the database sorts
/// natively via ORDER BY in the SQL query.
#[driver_test(requires(sql))]
pub async fn scan_order_by_sql(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        score: i64,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { score: 10_i64 },
        { score: 30_i64 },
        { score: 20_i64 },
        { score: 50_i64 },
        { score: 40_i64 },
    ])
    .exec(&mut db)
    .await?;

    let results: Vec<Item> = Item::all()
        .order_by(Item::fields().score().desc())
        .exec(&mut db)
        .await?;

    assert_eq!(5, results.len());
    for i in 0..4 {
        assert!(
            results[i].score >= results[i + 1].score,
            "expected descending order: results[{}].score={} >= results[{}].score={}",
            i,
            results[i].score,
            i + 1,
            results[i + 1].score
        );
    }

    Ok(())
}
