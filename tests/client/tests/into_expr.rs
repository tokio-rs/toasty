use tests_client::*;

async fn string_to_id_expr(s: impl Setup) {
    schema!(
        "
        model Foo {
            #[key]
            #[auto]
            id: Id,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let foo = db::Foo::create().exec(&db).await.unwrap();

    let id = foo.id.to_string();

    // Find the record using the ID as a &String
    let foo2 = db::Foo::get_by_id(&db, &id).await.unwrap();
    assert_eq!(foo2.id, foo.id);

    // Find the record using the ID as a &str
    let foo2 = db::Foo::get_by_id(&db, &id[..]).await.unwrap();
    assert_eq!(foo2.id, foo.id);

    // Find the record using the ID as a String
    let foo2 = db::Foo::get_by_id(&db, id).await.unwrap();
    assert_eq!(foo2.id, foo.id);
}

tests!(string_to_id_expr,);
