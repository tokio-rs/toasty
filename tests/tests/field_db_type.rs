use tests::*;

use toasty::stmt::Id;

async fn specify_constrained_string_field(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[db(varchar(5))]
        name: String,
    }

    let db = s.setup(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await.unwrap();
    assert_eq!(u.name, "foo");

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
}

tests!(specify_constrained_string_field,);
