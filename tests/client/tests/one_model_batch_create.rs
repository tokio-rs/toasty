use tests_client::*;

schema!(
    "
    model Todo {
        #[key]
        #[auto]
        id: Id,

        title: String,
    }"
);

async fn batch_create_empty(s: impl Setup) {
    let db = s.setup(db::load_schema()).await;

    let res = db::Todo::create_many().exec(&db).await.unwrap();
    assert!(res.is_empty());
}

async fn batch_create_one(s: impl Setup) {
    let db = s.setup(db::load_schema()).await;

    let res = db::Todo::create_many()
        .item(db::Todo::create().title("hello"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(1, res.len());

    assert_eq!(res[0].title, "hello");

    let reloaded: Vec<_> = db::Todo::find_by_id(&res[0].id).collect(&db).await.unwrap();
    assert_eq!(1, reloaded.len());
    assert_eq!(reloaded[0].id, res[0].id);
}

async fn batch_create_many(s: impl Setup) {
    let db = s.setup(db::load_schema()).await;

    let res = db::Todo::create_many()
        .item(db::Todo::create().title("todo 1"))
        .item(db::Todo::create().title("todo 2"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(2, res.len());

    assert_eq!(res[0].title, "todo 1");
    assert_eq!(res[1].title, "todo 2");

    for todo in &res {
        let reloaded: Vec<_> = db::Todo::find_by_id(&todo.id).collect(&db).await.unwrap();
        assert_eq!(1, reloaded.len());
        assert_eq!(reloaded[0].id, todo.id);
    }
}

// TODO: is a batch supposed to be atomic? Probably not.
async fn batch_create_fails_if_any_record_missing_fields(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            email: String,

            name: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let res = db::User::create_many()
        .item(db::User::create().email("user1@example.com").name("User 1"))
        .item(db::User::create().email("user2@example.com"))
        .exec(&db)
        .await
        .unwrap();

    assert!(res.is_empty());

    let users: Vec<_> = db::User::find_by_email("me@carllerche.com")
        .collect(&db)
        .await
        .unwrap();

    assert!(users.is_empty());
}

async fn batch_create_model_with_unique_field_index_all_unique(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            email: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let mut res = db::User::create_many()
        .item(db::User::create().email("user1@example.com"))
        .item(db::User::create().email("user2@example.com"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(2, res.len());

    res.sort_by_key(|user| user.email.clone());

    assert_eq!(res[0].email, "user1@example.com");
    assert_eq!(res[1].email, "user2@example.com");

    // We can fetch the user by ID and email
    for user in &res {
        let found = db::User::find_by_id(&user.id).get(&db).await.unwrap();
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);

        let found = db::User::find_by_email(&user.email).get(&db).await.unwrap();
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);
    }
}

async fn batch_create_model_with_unique_field_index_all_dups(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            email: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let _res = db::User::create_many()
        .item(db::User::create().email("user@example.com"))
        .item(db::User::create().email("user@example.com"))
        .exec(&db)
        .await
        .unwrap();
}

tests!(
    batch_create_empty,
    batch_create_one,
    batch_create_many,
    #[should_panic] // TODO: should it panic?
    batch_create_fails_if_any_record_missing_fields,
    batch_create_model_with_unique_field_index_all_unique,
    #[should_panic] // TODO: probaby shouldn't actually panic?
    batch_create_model_with_unique_field_index_all_dups,
);
