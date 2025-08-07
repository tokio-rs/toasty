use tests::{models, tests, DbTest, Setup};
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

tests!(string_to_id_expr,);
