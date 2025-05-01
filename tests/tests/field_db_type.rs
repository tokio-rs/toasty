use tests::*;

use toasty::stmt::Id;

async fn specify_constrained_string_field(s: impl Setup) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[db(varchar(5))]
        name: String,
    }

    if s.capability().storage_types.varchar.is_none() {
        return;
    }

    let db = s.setup(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await.unwrap();
    assert_eq!(u.name, "foo");

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
}

async fn specify_invalid_varchar_size(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[db(varchar(1_000_000_000_000))]
        name: String,
    }

    let Some(max) = s.capability().storage_types.varchar else {
        return;
    };

    if max >= 1_000_000_000_000 {
        return;
    }

    let err = assert_err!(s.connect(models!(User)).await);
    assert_eq!(
        err.to_string(),
        format!("max varchar capacity exceeded: 1000000000000 > {max}")
    );
}

async fn specify_varchar_ty_when_not_supported(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[db(varchar(5))]
        name: String,
    }

    if let Some(_) = s.capability().storage_types.varchar {
        return;
    }

    let err = assert_err!(s.connect(models!(User)).await);
    assert_eq!(err.to_string(), "varchar storage type not supported");
}

tests!(
    specify_constrained_string_field,
    specify_invalid_varchar_size,
    specify_varchar_ty_when_not_supported,
);
