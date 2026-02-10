//! Test batch creation of models

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn batch_create_empty(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        title: String,
    }

    let db = test.setup_db(models!(Todo)).await;

    let res = Todo::create_many().exec(&db).await.unwrap();
    assert!(res.is_empty());
}

#[driver_test(id(ID))]
pub async fn batch_create_one(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,
    }

    let db = test.setup_db(models!(Todo)).await;

    let res = Todo::create_many()
        .item(Todo::create().title("hello"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(1, res.len());

    assert_eq!(res[0].title, "hello");

    let reloaded: Vec<_> = Todo::filter_by_id(res[0].id).collect(&db).await.unwrap();
    assert_eq!(1, reloaded.len());
    assert_eq!(reloaded[0].id, res[0].id);
}

#[driver_test(id(ID))]
pub async fn batch_create_many(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,
    }

    let db = test.setup_db(models!(Todo)).await;

    let res = Todo::create_many()
        .item(Todo::create().title("todo 1"))
        .item(Todo::create().title("todo 2"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(2, res.len());

    assert_eq!(res[0].title, "todo 1");
    assert_eq!(res[1].title, "todo 2");

    for todo in &res {
        let reloaded: Vec<_> = Todo::filter_by_id(todo.id).collect(&db).await.unwrap();
        assert_eq!(1, reloaded.len());
        assert_eq!(reloaded[0].id, todo.id);
    }
}

// TODO: is a batch supposed to be atomic? Probably not.
#[driver_test(id(ID))]
#[should_panic]
pub async fn batch_create_fails_if_any_record_missing_fields(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        email: String,

        #[allow(dead_code)]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let res = User::create_many()
        .item(User::create().email("user1@example.com").name("User 1"))
        .item(User::create().email("user2@example.com"))
        .exec(&db)
        .await
        .unwrap();

    assert!(res.is_empty());

    let users: Vec<_> = User::filter_by_email("me@carllerche.com")
        .collect(&db)
        .await
        .unwrap();

    assert!(users.is_empty());
}

#[driver_test(id(ID))]
pub async fn batch_create_model_with_unique_field_index_all_unique(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,
    }

    let db = test.setup_db(models!(User)).await;

    let mut res = User::create_many()
        .item(User::create().email("user1@example.com"))
        .item(User::create().email("user2@example.com"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(2, res.len());

    res.sort_by_key(|user| user.email.clone());

    assert_eq!(res[0].email, "user1@example.com");
    assert_eq!(res[1].email, "user2@example.com");

    // We can fetch the user by ID and email
    for user in &res {
        let found = User::get_by_id(&db, user.id).await.unwrap();
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);

        let found = User::get_by_email(&db, &user.email).await.unwrap();
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);
    }
}

#[driver_test(id(ID))]
#[should_panic]
pub async fn batch_create_model_with_unique_field_index_all_dups(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        #[allow(dead_code)]
        email: String,
    }

    let db = test.setup_db(models!(User)).await;

    let _res = User::create_many()
        .item(User::create().email("user@example.com"))
        .item(User::create().email("user@example.com"))
        .exec(&db)
        .await
        .unwrap();
}
