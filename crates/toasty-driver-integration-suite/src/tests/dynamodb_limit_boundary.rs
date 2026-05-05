//! DynamoDB 1 MB response boundary tests.
//!
//! These tests prove the paging loop works correctly when the result set spans
//! DynamoDB's 1 MB response boundary. DynamoDB's Query API returns at most 1 MB
//! of data per call and sets `LastEvaluatedKey` when there are more results.
//! With ~10 KB items, ~100 items ≈ 1 MB, so seeding 200 items and querying with
//! `.limit(150)` forces at least 2 DynamoDB API calls, exercising the pagination
//!  loop in the driver.
//!
//! IMPORTANT: The `payload` field is intentionally large (10,000 bytes). Do NOT
//! reduce its size. The tests depend on the payload being large enough to push
//! each batch of ~100 items past the 1 MB boundary so that at least two DynamoDB
//! API calls are required to satisfy the limit.

use crate::prelude::*;

// ── Base table tests (composite partition + sort key) ────────────────────────

#[driver_test(requires(not(sql)))]
pub async fn limit_spans_page_boundary(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Item {
        kind: String,
        seq: i64,
        payload: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let payload = "x".repeat(10_000);
    for i in 0..200_i64 {
        toasty::create!(Item {
            kind: "boundary",
            seq: i,
            payload: payload.clone(),
        })
        .exec(&mut db)
        .await?;
    }

    let items: Vec<_> = Item::filter_by_kind("boundary")
        .order_by(Item::fields().seq().asc())
        .limit(150)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 150);
    assert_eq!(items[0].seq, 0);
    assert_eq!(items[149].seq, 149);

    Ok(())
}

#[driver_test(requires(not(sql)))]
pub async fn limit_offset_spans_page_boundary(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Item {
        kind: String,
        seq: i64,
        payload: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let payload = "x".repeat(10_000);
    for i in 0..200_i64 {
        toasty::create!(Item {
            kind: "boundary",
            seq: i,
            payload: payload.clone(),
        })
        .exec(&mut db)
        .await?;
    }

    let items: Vec<_> = Item::filter_by_kind("boundary")
        .order_by(Item::fields().seq().asc())
        .limit(100)
        .offset(50)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 100);
    assert_eq!(items[0].seq, 50);
    assert_eq!(items[99].seq, 149);

    Ok(())
}

/// No-limit query across a 1 MB boundary returns **all** rows.
///
/// With ~10 KB items and 200 rows (~2 MB total), a single DynamoDB `Query`
/// call is capped at 1 MB and returns only ~100 rows. The driver must follow
/// `LastEvaluatedKey` and keep querying until all results are returned.
#[driver_test(requires(not(sql)))]
pub async fn no_limit_spans_page_boundary(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Item {
        kind: String,
        seq: i64,
        payload: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let payload = "x".repeat(10_000);
    for i in 0..200_i64 {
        toasty::create!(Item {
            kind: "boundary",
            seq: i,
            payload: payload.clone(),
        })
        .exec(&mut db)
        .await?;
    }

    let items: Vec<_> = Item::filter_by_kind("boundary").exec(&mut db).await?;

    assert_eq!(items.len(), 200);

    Ok(())
}

// ── GSI tests (non-unique index on a UUID-keyed model) ────────────────────────

#[driver_test(requires(not(sql)))]
pub async fn limit_spans_page_boundary_gsi(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct GsiItem {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        category: String,

        seq: i64,
        payload: String,
    }

    let mut db = t.setup_db(models!(GsiItem)).await;

    let payload = "x".repeat(10_000);
    for i in 0..200_i64 {
        toasty::create!(GsiItem {
            category: "boundary",
            seq: i,
            payload: payload.clone(),
        })
        .exec(&mut db)
        .await?;
    }

    // DDB GSI ordering is only guaranteed on the GSI sort key; seq is not the
    // sort key here, so only assert the count.
    let items: Vec<_> = GsiItem::filter_by_category("boundary")
        .limit(150)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 150);

    Ok(())
}

#[driver_test(requires(not(sql)))]
pub async fn limit_offset_spans_page_boundary_gsi(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct GsiItem {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        category: String,

        seq: i64,
        payload: String,
    }

    let mut db = t.setup_db(models!(GsiItem)).await;

    let payload = "x".repeat(10_000);
    for i in 0..200_i64 {
        toasty::create!(GsiItem {
            category: "boundary",
            seq: i,
            payload: payload.clone(),
        })
        .exec(&mut db)
        .await?;
    }

    // DDB GSI ordering is only guaranteed on the GSI sort key; seq is not the
    // sort key here, so only assert the count.
    let items: Vec<_> = GsiItem::filter_by_category("boundary")
        .limit(100)
        .offset(50)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 100);

    Ok(())
}
