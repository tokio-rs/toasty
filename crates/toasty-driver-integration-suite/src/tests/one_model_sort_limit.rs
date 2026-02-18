//! Test sorting and pagination of query results

use crate::prelude::*;
use toasty::Page;

#[driver_test(id(ID))]
pub async fn sort_asc(test: &mut Test) -> Result<()> {
    if !test.capability().sql {
        return Ok(());
    }

    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let db = test.setup_db(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&db).await?;
    }

    let foos_asc: Vec<_> = Foo::all()
        .order_by(Foo::fields().order().asc())
        .collect(&db)
        .await?;

    assert_eq!(foos_asc.len(), 100);

    for i in 0..99 {
        assert!(foos_asc[i].order < foos_asc[i + 1].order);
    }

    let foos_desc: Vec<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .collect(&db)
        .await?;

    assert_eq!(foos_desc.len(), 100);

    for i in 0..99 {
        assert!(foos_desc[i].order > foos_desc[i + 1].order);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn paginate(test: &mut Test) -> Result<()> {
    if !test.capability().sql {
        return Ok(());
    }

    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        order: i64,
    }

    let db = test.setup_db(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&db).await?;
    }

    let foos: Page<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .paginate(10)
        .collect(&db)
        .await?;

    assert_eq!(foos.len(), 10);
    for (i, order) in (90..100).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = Foo::all()
        .order_by(Foo::fields().order().desc())
        .paginate(10)
        .after(90)
        .collect(&db)
        .await?;

    assert_eq!(foos.len(), 10);
    for (i, order) in (80..90).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = foos.next(&db).await?.unwrap();
    assert_eq!(foos.len(), 10);
    for (i, order) in (70..80).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = foos.prev(&db).await?.unwrap();
    assert_eq!(foos.len(), 10);
    for (i, order) in (80..90).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Page<_> = foos.next(&db).await?.unwrap();
    assert_eq!(foos.len(), 10);
    for (i, order) in (70..80).rev().enumerate() {
        assert_eq!(foos[i].order, order);
    }
    Ok(())
}
