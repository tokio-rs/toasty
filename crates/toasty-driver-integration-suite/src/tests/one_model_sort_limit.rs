//! Test sorting and pagination of query results

use crate::prelude::*;
use toasty::Page;
use toasty_core::{
    driver::{Operation, Rows},
    stmt::{ExprSet, Statement},
};

#[driver_test(id(ID), requires(sql))]
pub async fn sort_asc(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let mut db = test.setup_db(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&mut db).await?;
    }

    test.log().clear();

    let foos_asc: Vec<_> = Foo::all()
        .order_by(Foo::fields().order().asc())
        .collect(&mut db)
        .await?;

    assert_eq!(foos_asc.len(), 100);

    for i in 0..99 {
        assert!(foos_asc[i].order < foos_asc[i + 1].order);
    }

    // Verify the SQL query has an ORDER BY clause
    let (op, resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ { .. }),
            order_by: Some(_),
            ..
        }),
        ..
    }));
    assert_struct!(resp.rows, Rows::Stream(_));

    test.log().clear();

    let foos_desc: Vec<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .collect(&mut db)
        .await?;

    assert_eq!(foos_desc.len(), 100);

    for i in 0..99 {
        assert!(foos_desc[i].order > foos_desc[i + 1].order);
    }

    let (op, resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ { .. }),
            order_by: Some(_),
            ..
        }),
        ..
    }));
    assert_struct!(resp.rows, Rows::Stream(_));

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn paginate(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let mut db = test.setup_db(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&mut db).await?;
    }

    test.log().clear();

    let foos: Page<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .paginate(10)
        .collect(&mut db)
        .await?;

    assert_eq!(foos.len(), 10);
    for (i, order) in (90..100).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    // First page: SQL query should have ORDER BY and LIMIT
    let (op, resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ { .. }),
            order_by: Some(_),
            limit: Some(_),
            ..
        }),
        ..
    }));
    assert_struct!(resp.rows, Rows::Stream(_));

    test.log().clear();

    let foos: Page<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .paginate(10)
        .after(90)
        .collect(&mut db)
        .await?;

    assert_eq!(foos.len(), 10);
    for (i, order) in (80..90).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = foos.next(&mut db).await?.unwrap();
    assert_eq!(foos.len(), 10);
    for (i, order) in (70..80).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = foos.prev(&mut db).await?.unwrap();
    assert_eq!(foos.len(), 10);
    for (i, order) in (80..90).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = foos.next(&mut db).await?.unwrap();
    assert_eq!(foos.len(), 10);
    for (i, order) in (70..80).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn limit(t: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let mut db = t.setup_db(models!(Foo)).await;

    for i in 0..20 {
        Foo::create().order(i).exec(&mut db).await?;
    }

    t.log().clear();

    // Basic limit without ordering
    let foos: Vec<_> = Foo::all().limit(5).collect(&mut db).await?;
    assert_eq!(foos.len(), 5);

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ { .. }),
            limit: Some(_),
            ..
        }),
        ..
    }));

    t.log().clear();

    // Limit combined with ordering
    let foos: Vec<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .limit(7)
        .collect(&mut db)
        .await?;
    assert_eq!(foos.len(), 7);
    for i in 0..6 {
        assert!(foos[i].order > foos[i + 1].order);
    }

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ { .. }),
            order_by: Some(_),
            limit: Some(_),
            ..
        }),
        ..
    }));

    t.log().clear();

    // Limit larger than the result set returns all results
    let foos: Vec<_> = Foo::all().limit(100).collect(&mut db).await?;
    assert_eq!(foos.len(), 20);

    Ok(())
}
