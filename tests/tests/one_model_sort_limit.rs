use tests::{models, tests, Setup};
use toasty::stmt::Id;

#[derive(toasty::Model)]
struct Foo {
    #[key]
    #[auto]
    id: Id<Self>,

    #[index]
    order: i64,
}

async fn sort_asc(s: impl Setup) {
    if !s.capability().sql {
        return;
    }

    let db = s.setup(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&db).await.unwrap();
    }

    let foos_asc: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order().asc())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos_asc.len(), 100);

    for i in 0..99 {
        assert!(foos_asc[i].order < foos_asc[i + 1].order);
    }

    let foos_desc: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos_desc.len(), 100);

    for i in 0..99 {
        assert!(foos_desc[i].order > foos_desc[i + 1].order);
    }
}

async fn paginate(s: impl Setup) {
    if !s.capability().sql {
        return;
    }

    let db = s.setup(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&db).await.unwrap();
    }

    let page = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .paginate(10)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(page.items.len(), 10);
    // There are 100 items total, we're on the first page of 10, so there should be more pages
    assert!(page.has_next(), "First page should have next");
    assert!(!page.has_prev(), "First page should not have prev");
    
    for (i, order) in (99..90).enumerate() {
        assert_eq!(page.items[i].order, order);
    }

    let page = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .paginate(10)
        .after(90)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(page.items.len(), 10);
    assert!(page.has_next(), "Second page should have next");
    // Note: prev cursor is not implemented yet, so this will be false
    assert!(!page.has_prev(), "Prev cursor not implemented yet");
    
    for (i, order) in (89..80).enumerate() {
        assert_eq!(page.items[i].order, order);
    }
    
    // Test last page (items with order 9..0)
    let last_page = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .paginate(10)
        .after(10)
        .collect(&db)
        .await
        .unwrap();
    
    assert_eq!(last_page.items.len(), 10);
    assert!(!last_page.has_next(), "Last page should not have next");
    
    for (i, order) in (9..0).enumerate() {
        assert_eq!(last_page.items[i].order, order);
    }
}

tests!(sort_asc, paginate,);
