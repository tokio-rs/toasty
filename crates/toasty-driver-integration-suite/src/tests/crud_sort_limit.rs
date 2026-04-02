//! Test sorting and pagination of query results

use crate::prelude::*;
use toasty::stmt::Page;
use toasty_core::{
    driver::{Operation, Rows},
    stmt::{ExprSet, Statement},
};

#[driver_test(id(ID), requires(sql))]
pub async fn sort_asc(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    for i in 0..100 {
        Item::create().order(i).exec(&mut db).await?;
    }

    test.log().clear();

    let items_asc: Vec<_> = Item::all()
        .order_by(Item::fields().order().asc())
        .exec(&mut db)
        .await?;

    assert_eq!(items_asc.len(), 100);

    for i in 0..99 {
        assert!(items_asc[i].order < items_asc[i + 1].order);
    }

    // Verify the SQL query has an ORDER BY clause
    let (op, resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Query({
            body: ExprSet::Select({ .. }),
            order_by: Some(_),
        }),
    }));
    assert_struct!(resp.values, Rows::Stream(_));

    test.log().clear();

    let items_desc: Vec<_> = Item::all()
        .order_by(Item::fields().order().desc())
        .exec(&mut db)
        .await?;

    assert_eq!(items_desc.len(), 100);

    for i in 0..99 {
        assert!(items_desc[i].order > items_desc[i + 1].order);
    }

    let (op, resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Query({
            body: ExprSet::Select({ .. }),
            order_by: Some(_),
        }),
    }));
    assert_struct!(resp.values, Rows::Stream(_));

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn paginate(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    for i in 0..100 {
        Item::create().order(i).exec(&mut db).await?;
    }

    test.log().clear();

    let items: Page<_> = Item::all()
        .order_by(Item::fields().order().desc())
        .paginate(10)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 10);
    for (i, order) in (90..100).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }

    // First page: SQL query should have ORDER BY and LIMIT
    let (op, resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Query({
            body: ExprSet::Select({ .. }),
            order_by: Some(_),
            limit: Some(_),
        }),
    }));
    assert_struct!(resp.values, Rows::Stream(_));

    test.log().clear();

    let items: Page<_> = Item::all()
        .order_by(Item::fields().order().desc())
        .paginate(10)
        .after(90)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 10);
    for (i, order) in (80..90).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }

    let items: Page<_> = items.next(&mut db).await?.unwrap();
    assert_eq!(items.len(), 10);
    for (i, order) in (70..80).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }

    let items: Page<_> = items.prev(&mut db).await?.unwrap();
    assert_eq!(items.len(), 10);
    for (i, order) in (80..90).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }

    let items: Page<_> = items.next(&mut db).await?.unwrap();
    assert_eq!(items.len(), 10);
    for (i, order) in (70..80).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn limit_offset(t: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let mut db = t.setup_db(toasty::models!(Item)).await;

    for i in 0..20 {
        Item::create().order(i).exec(&mut db).await?;
    }

    t.log().clear();

    // Basic limit without ordering
    let items: Vec<_> = Item::all().limit(5).exec(&mut db).await?;
    assert_eq!(items.len(), 5);

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Query({
            body: ExprSet::Select({ .. }),
            limit: Some(_),
        }),
    }));

    t.log().clear();

    // Limit combined with ordering
    let items: Vec<_> = Item::all()
        .order_by(Item::fields().order().desc())
        .limit(7)
        .exec(&mut db)
        .await?;
    assert_eq!(items.len(), 7);
    for i in 0..6 {
        assert!(items[i].order > items[i + 1].order);
    }

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Query({
            body: ExprSet::Select({ .. }),
            order_by: Some(_),
            limit: Some(_),
        }),
    }));

    t.log().clear();

    // Limit combined with offset
    let items: Vec<_> = Item::all()
        .order_by(Item::fields().order().asc())
        .limit(7)
        .offset(5)
        .exec(&mut db)
        .await?;
    assert_eq!(items.len(), 7);
    for (i, f) in items.iter().enumerate() {
        assert_eq!(f.order, i as i64 + 5);
    }

    // Limit larger than the result set returns all results
    let items: Vec<_> = Item::all().limit(100).exec(&mut db).await?;
    assert_eq!(items.len(), 20);

    Ok(())
}

#[driver_test(requires(not(sql)))]
pub async fn paginate_for_dynamodb(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    #[key(id, order)]
    struct Item {
        id: i64,

        order: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for id in 0..2 {
        for order in 0..50 {
            Item::create().id(id).order(order).exec(&mut db).await?;
        }
    }

    test.log().clear();

    let items: Page<_> = Item::all()
        .order_by(Item::fields().order().desc())
        .filter(Item::fields().id().eq(0))
        .paginate(10)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 10);
    for (i, order) in (40..50).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }

    let mut items: Page<_> = items.next(&mut db).await?.unwrap();
    assert_eq!(items.len(), 10);
    for (i, order) in (30..40).rev().enumerate() {
        assert_eq!(items[i].order, order);
    }

    let mut count = 0;
    while let Some(i) = items.next(&mut db).await? {
        count += i.items.len();
        items = i;
    }
    assert_eq!(count, 30);

    let mut items: Page<_> = Item::all()
        .order_by(Item::fields().order().asc())
        .filter(Item::fields().id().eq(0))
        .paginate(10)
        .exec(&mut db)
        .await?;

    let mut count = items.items.len();
    while let Some(i) = items.next(&mut db).await? {
        count += i.items.len();
        items = i;
    }
    assert_eq!(count, 50);

    Ok(())
}
