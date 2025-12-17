use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn string_to_id_expr(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = test.setup_db(models!(Foo)).await;

    #[allow(clippy::disallowed_names)]
    let foo = Foo::create().exec(&db).await.unwrap();

    let id = foo.id.to_string();

    // Find the record using the ID as a &String
    let foo2 = Foo::get_by_id(&db, &id).await.unwrap();
    assert_eq!(foo2.id, foo.id);

    // Find the record using the ID as a &str
    let foo2 = Foo::get_by_id(&db, &id[..]).await.unwrap();
    assert_eq!(foo2.id, foo.id);

    // Find the record using the ID as a String
    let foo2 = Foo::get_by_id(&db, id).await.unwrap();
    assert_eq!(foo2.id, foo.id);
}

async fn numeric_id_expr(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        id: Id<Self>,
    }

    let db = test.setup_db(models!(Foo)).await;

    Foo::create().id("1").exec(&db).await.unwrap();
    Foo::create().id("2").exec(&db).await.unwrap();

    let foo = Foo::get_by_id(&db, 1i32).await.unwrap();
    assert_eq!(foo.id.to_string(), "1");

    let foo = Foo::get_by_id(&db, 2u64).await.unwrap();
    assert_eq!(foo.id.to_string(), "2");

    let foo = Foo::get_by_id(&db, 2usize).await.unwrap();
    assert_eq!(foo.id.to_string(), "2");

    let explicit = Id::<Foo>::from_u64(1);
    let foo = Foo::get_by_id(&db, &explicit).await.unwrap();
    assert_eq!(foo.id.to_string(), "1");

    let foo = Foo::get_by_id(&db, 1isize).await.unwrap();
    assert_eq!(foo.id.to_string(), "1");
}

tests!(string_to_id_expr, numeric_id_expr,);
