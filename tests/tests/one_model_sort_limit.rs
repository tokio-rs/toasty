use tests::*;

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
        .order_by(Foo::FIELDS.order.asc())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos_asc.len(), 100);

    for i in 0..99 {
        assert!(foos_asc[i].order < foos_asc[i + 1].order);
    }

    let foos_desc: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order.desc())
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

    let foos: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order.desc())
        .paginate(10)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos.len(), 10);
    for (i, order) in (99..90).enumerate() {
        assert_eq!(foos[i].order, order);
    }

    let foos: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order.desc())
        .paginate(10)
        .after(90)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos.len(), 10);
    for (i, order) in (89..80).enumerate() {
        assert_eq!(foos[i].order, order);
    }
}

tests!(sort_asc, paginate,);
