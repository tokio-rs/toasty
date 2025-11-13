use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn specify_custom_column_name(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[column("my_name")]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await.unwrap();
    assert_eq!(u.name, "foo");

    // Make sure the column has actually been renamed to my_name in the underlying database.
    assert_eq!(
        test.get_raw_column_value::<String>("users", "my_name", Default::default())
            .await
            .unwrap(),
        "foo"
    );
}

async fn specify_custom_column_name_with_type(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[column("my_name", type = varchar(5))]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await.unwrap();
    assert_eq!(u.name, "foo");

    // Make sure the column has actually been renamed to my_name in the underlying database.
    assert_eq!(
        test.get_raw_column_value::<String>("users", "my_name", Default::default())
            .await
            .unwrap(),
        "foo"
    );

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
}

tests!(
    specify_custom_column_name,
    specify_custom_column_name_with_type
);
