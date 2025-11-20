use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn auto_uuid_v4(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[auto(uuid(v4))]
        auto_field: uuid::Uuid,
    }

    let db = test.setup_db(models!(Foo)).await;

    let u = Foo::create().exec(&db).await.unwrap();
    // Sanity check that it actually generated a UUID
    assert!(uuid::Uuid::parse_str(&u.auto_field.to_string()).is_ok());
}

async fn auto_uuid_v7(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[auto(uuid(v7))]
        auto_field: uuid::Uuid,
    }

    let db = test.setup_db(models!(Foo)).await;

    let u = Foo::create().exec(&db).await.unwrap();
    // Sanity check that it actually generated a UUID
    assert!(uuid::Uuid::parse_str(&u.auto_field.to_string()).is_ok());
}

async fn auto_increment(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct Foo {
        #[key]
        #[auto(increment)]
        auto_field: u32,
    }

    let db = test.setup_db(models!(Foo)).await;

    for i in 0..10 {
        let u = Foo::create().exec(&db).await.unwrap();
        assert_eq!(u.auto_field, i);
    }
}

tests!(auto_uuid_v4, auto_uuid_v7, auto_increment);
