//! Test batch creation of models

use crate::prelude::*;

use toasty::Executor;
use toasty_core::driver::{Operation, operation::Transaction};

#[driver_test(id(ID))]
pub async fn batch_create_empty(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        title: String,
    }

    let mut db = test.setup_db(models!(Todo)).await;

    let res = Todo::create_many().exec(&mut db).await?;
    assert!(res.is_empty());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn batch_create_one(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,
    }

    let mut db = test.setup_db(models!(Todo)).await;

    test.log().clear();
    let res = Todo::create_many()
        .item(Todo::create().title("hello"))
        .exec(&mut db)
        .await?;

    assert_eq!(1, res.len());
    assert_eq!(res[0].title, "hello");

    // Single-row batch: no transaction wrapping needed
    if test.capability().sql {
        assert_struct!(test.log().pop_op(), Operation::QuerySql(_));
        assert!(test.log().is_empty());
    }

    let reloaded: Vec<_> = Todo::filter_by_id(res[0].id).exec(&mut db).await?;
    assert_eq!(1, reloaded.len());
    assert_eq!(reloaded[0].id, res[0].id);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn batch_create_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,
    }

    let mut db = test.setup_db(models!(Todo)).await;

    test.log().clear();
    let res = Todo::create_many()
        .item(Todo::create().title("todo 1"))
        .item(Todo::create().title("todo 2"))
        .exec(&mut db)
        .await?;

    assert_eq!(2, res.len());
    assert_eq!(res[0].title, "todo 1");
    assert_eq!(res[1].title, "todo 2");

    // Multi-row batch in a single INSERT statement: no transaction wrapping
    // needed because single SQL statements are inherently atomic.
    if test.capability().sql {
        assert_struct!(test.log().pop_op(), Operation::QuerySql(_));
        assert!(test.log().is_empty());
    }

    for todo in &res {
        let reloaded: Vec<_> = Todo::filter_by_id(todo.id).exec(&mut db).await?;
        assert_eq!(1, reloaded.len());
        assert_eq!(reloaded[0].id, todo.id);
    }
    Ok(())
}

// TODO: is a batch supposed to be atomic? Probably not.
#[driver_test(id(ID))]
#[should_panic]
pub async fn batch_create_fails_if_any_record_missing_fields(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        email: String,

        #[allow(dead_code)]
        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    let res = User::create_many()
        .item(User::create().email("user1@example.com").name("User 1"))
        .item(User::create().email("user2@example.com"))
        .exec(&mut db)
        .await?;

    assert!(res.is_empty());

    let users: Vec<_> = User::filter_by_email("me@carllerche.com")
        .exec(&mut db)
        .await?;

    assert!(users.is_empty());
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::user_unique_email))]
pub async fn batch_create_model_with_unique_field_index_all_unique(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut res = User::create_many()
        .item(User::create().email("user1@example.com"))
        .item(User::create().email("user2@example.com"))
        .exec(&mut db)
        .await?;

    assert_eq!(2, res.len());

    res.sort_by_key(|user| user.email.clone());

    assert_eq!(res[0].email, "user1@example.com");
    assert_eq!(res[1].email, "user2@example.com");

    // We can fetch the user by ID and email
    for user in &res {
        let found = User::get_by_id(&mut db, user.id).await?;
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);

        let found = User::get_by_email(&mut db, &user.email).await?;
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);
    }
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::user_unique_email))]
#[should_panic]
pub async fn batch_create_model_with_unique_field_index_all_dups(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let _res = User::create_many()
        .item(User::create().email("user@example.com"))
        .item(User::create().email("user@example.com"))
        .exec(&mut db)
        .await?;
    Ok(())
}

/// Unique constraint violation on a multi-row batch is atomic because a single
/// INSERT statement is inherently atomic in SQL databases.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_unique_email))]
pub async fn batch_create_unique_violation_rolls_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    // Seed the duplicate
    User::create()
        .email("taken@example.com")
        .exec(&mut db)
        .await?;

    t.log().clear();
    assert_err!(
        User::create_many()
            .item(User::create().email("new@example.com"))
            .item(User::create().email("taken@example.com"))
            .exec(&mut db)
            .await
    );

    // No transaction wrapper — the single INSERT fails atomically
    assert!(t.log().is_empty());

    // Only the seeded user remains
    let users = User::all().exec(&mut db).await?;
    assert_eq!(1, users.len());

    Ok(())
}

/// Multi-row batch inside an explicit transaction executes as a single INSERT
/// without extra savepoint wrapping (the statement is inherently atomic).
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_inside_transaction_uses_savepoints(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        title: String,
    }

    let mut db = t.setup_db(models!(Todo)).await;

    t.log().clear();
    let mut tx = db.transaction().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );

    Todo::create_many()
        .item(Todo::create().title("a"))
        .item(Todo::create().title("b"))
        .exec(&mut tx)
        .await?;

    // Single INSERT statement — no savepoint needed
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));

    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}
