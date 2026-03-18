//! Test pagination on composite-key models.
//!
//! These tests exercise `paginate()`, `limit()`, and `order_by()` on models
//! with a partition + local key, which is the pattern DynamoDB uses for
//! `QueryPk`. They intentionally have **no** `requires(sql)` gate so they run
//! on every driver, including DynamoDB.

use crate::prelude::*;
use toasty::Page;
use toasty_core::driver::{Operation, Rows};

#[driver_test]
pub async fn paginate_composite_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Event {
        kind: String,
        seq: i64,
    }

    let mut db = test.setup_db(models!(Event)).await;

    // Seed 20 events under the same partition key so we can paginate over them.
    for i in 0..20 {
        Event::create().kind("info").seq(i).exec(&mut db).await?;
    }

    test.log().clear();
    eprintln!("Post create");
    // First page (descending): should return seq 19..10
    let page: Page<_> = Event::filter_by_kind("info")
        .order_by(Event::fields().seq().desc())
        .paginate(10)
        .exec(&mut db)
        .await?;

    assert_eq!(page.len(), 10);
    for (i, expected) in (10..20).rev().enumerate() {
        assert_eq!(page[i].seq, expected);
    }

    // Verify the driver operation type
    let (op, resp) = test.log().pop();
    if test.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }
    assert_struct!(resp.rows, Rows::Stream(_));

    // Second page via .next()
    eprintln!("About to crash");
    let page: Page<_> = page.next(&mut db).await?.unwrap();
    eprintln!("Should've crashed");
    assert_eq!(page.len(), 10);
    for (i, expected) in (0..10).rev().enumerate() {
        assert_eq!(page[i].seq, expected);
    }

    // Go back to the first page via .prev()
    let page: Page<_> = page.prev(&mut db).await?.unwrap();
    assert_eq!(page.len(), 10);
    for (i, expected) in (10..20).rev().enumerate() {
        assert_eq!(page[i].seq, expected);
    }

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_composite_key_asc(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Event {
        kind: String,
        seq: i64,
    }

    let mut db = test.setup_db(models!(Event)).await;

    for i in 0..20 {
        Event::create().kind("info").seq(i).exec(&mut db).await?;
    }

    test.log().clear();

    // First page (ascending): should return seq 0..9
    let page: Page<_> = Event::filter_by_kind("info")
        .order_by(Event::fields().seq().asc())
        .paginate(5)
        .exec(&mut db)
        .await?;

    assert_eq!(page.len(), 5);
    for (i, expected) in (0..5).enumerate() {
        assert_eq!(page[i].seq, expected);
    }

    let (op, _) = test.log().pop();
    if test.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    // Walk forward through all pages and collect every seq value.
    let mut all_seqs: Vec<i64> = page.iter().map(|e| e.seq).collect();
    let mut current = page;
    while let Some(next) = current.next(&mut db).await? {
        all_seqs.extend(next.iter().map(|e| e.seq));
        current = next;
    }

    assert_eq!(all_seqs, (0..20).collect::<Vec<_>>());

    Ok(())
}

#[driver_test]
pub async fn limit_composite_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Event {
        kind: String,
        seq: i64,
    }

    let mut db = test.setup_db(models!(Event)).await;

    for i in 0..20 {
        Event::create().kind("info").seq(i).exec(&mut db).await?;
    }

    test.log().clear();

    // Limit without explicit ordering
    let events: Vec<_> = Event::filter_by_kind("info").limit(7).exec(&mut db).await?;
    assert_eq!(events.len(), 7);

    let (op, _) = test.log().pop();
    if test.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    test.log().clear();

    // Limit combined with descending order
    let events: Vec<_> = Event::filter_by_kind("info")
        .order_by(Event::fields().seq().desc())
        .limit(5)
        .exec(&mut db)
        .await?;
    assert_eq!(events.len(), 5);
    for i in 0..4 {
        assert!(events[i].seq > events[i + 1].seq);
    }
    // The first item should be the highest seq
    assert_eq!(events[0].seq, 19);

    test.log().clear();

    // Limit larger than result set returns all results
    let events: Vec<_> = Event::filter_by_kind("info")
        .limit(100)
        .exec(&mut db)
        .await?;
    assert_eq!(events.len(), 20);

    Ok(())
}

#[driver_test]
pub async fn sort_composite_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = seq)]
    struct Event {
        kind: String,
        seq: i64,
    }

    let mut db = test.setup_db(models!(Event)).await;

    for i in 0..20 {
        Event::create().kind("info").seq(i).exec(&mut db).await?;
    }

    test.log().clear();

    // Ascending sort
    let events: Vec<_> = Event::filter_by_kind("info")
        .order_by(Event::fields().seq().asc())
        .exec(&mut db)
        .await?;

    assert_eq!(events.len(), 20);
    for i in 0..19 {
        assert!(events[i].seq < events[i + 1].seq);
    }

    let (op, resp) = test.log().pop();
    if test.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }
    assert_struct!(resp.rows, Rows::Stream(_));

    test.log().clear();

    // Descending sort
    let events: Vec<_> = Event::filter_by_kind("info")
        .order_by(Event::fields().seq().desc())
        .exec(&mut db)
        .await?;

    assert_eq!(events.len(), 20);
    for i in 0..19 {
        assert!(events[i].seq > events[i + 1].seq);
    }

    let (op, resp) = test.log().pop();
    if test.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }
    assert_struct!(resp.rows, Rows::Stream(_));

    Ok(())
}
