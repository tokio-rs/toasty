use tests::{assert_err, models, tests, DbTest};
use toasty::stmt::Id;

async fn specify_constrained_string_field(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[column(type = varchar(5))]
        name: String,
    }

    if test.capability().storage_types.varchar.is_none() {
        return;
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await.unwrap();
    assert_eq!(u.name, "foo");

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
}

async fn specify_invalid_varchar_size(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[column(type = varchar(1_000_000_000_000))]
        name: String,
    }

    let Some(max) = test.capability().storage_types.varchar else {
        return;
    };

    if max >= 1_000_000_000_000 {
        return;
    }

    // Try to setup a database with an invalid varchar size
    let err = assert_err!(test.try_setup_db(models!(User)).await);
    assert_eq!(
        err.to_string(),
        format!("max varchar capacity exceeded: 1000000000000 > {max}")
    );
}

async fn specify_varchar_ty_when_not_supported(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[column(type = "varchar(5)")]
        name: String,
    }

    if test.capability().storage_types.varchar.is_some() {
        return;
    }

    // Try to setup a database with varchar when not supported
    let err = assert_err!(test.try_setup_db(models!(User)).await);
    assert_eq!(err.to_string(), "varchar storage type not supported");
}

tests!(
    specify_constrained_string_field,
    specify_invalid_varchar_size,
    specify_varchar_ty_when_not_supported,
);
