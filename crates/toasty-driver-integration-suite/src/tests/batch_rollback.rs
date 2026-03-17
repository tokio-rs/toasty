use crate::prelude::*;

use toasty_core::driver::{operation::Transaction, Operation};

/// When a batch of two creates fails on the second INSERT (unique constraint
/// violation), the entire batch is rolled back — the first INSERT must not
/// persist.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_creates_rolls_back_on_second_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    // Seed the name that will cause the second create to fail.
    User::create().name("taken").exec(&mut db).await?;

    t.log().clear();
    assert_err!(
        toasty::batch((
            User::create().name("new-user"),
            User::create().name("taken"),
        ))
        .exec(&mut db)
        .await
    );

    // BEGIN → INSERT (succeeds) → INSERT (fails, not logged) → ROLLBACK
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // first INSERT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // Only the seeded user remains — "new-user" was rolled back
    let users = User::all().collect::<Vec<_>>(&mut db).await?;
    assert_eq!(1, users.len());
    assert_eq!(users[0].name, "taken");

    Ok(())
}

/// When a batch of a create + update fails on the update (unique constraint),
/// the successful create is rolled back.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_and_update_rolls_back_on_update_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create().name("alice").exec(&mut db).await?;
    User::create().name("taken").exec(&mut db).await?;

    t.log().clear();
    assert_err!(
        toasty::batch((
            User::create().name("bob"),
            User::filter_by_name("alice").update().name("taken"), // fails: unique
        ))
        .exec(&mut db)
        .await
    );

    // BEGIN → INSERT bob (succeeds) → UPDATE alice (fails) → ROLLBACK
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // "bob" was rolled back and "alice" was not renamed
    let all = User::all().collect::<Vec<_>>(&mut db).await?;
    assert_eq!(2, all.len());
    let names: std::collections::HashSet<_> = all.iter().map(|u| u.name.as_str()).collect();
    assert!(names.contains("alice"));
    assert!(names.contains("taken"));

    Ok(())
}

/// When a batch of an update + create fails on the create (unique constraint),
/// the successful update is rolled back.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_update_and_create_rolls_back_on_create_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create().name("alice").exec(&mut db).await?;
    User::create().name("taken").exec(&mut db).await?;

    t.log().clear();
    assert_err!(
        toasty::batch((
            User::filter_by_name("alice").update().name("alice2"),
            User::create().name("taken"), // fails: unique
        ))
        .exec(&mut db)
        .await
    );

    // BEGIN → UPDATE (succeeds) → INSERT (fails) → ROLLBACK
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // UPDATE
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // Update was rolled back — "alice" still has her original name
    let alice: Vec<_> = User::filter_by_name("alice").collect(&mut db).await?;
    assert_eq!(1, alice.len());

    // No "alice2" exists
    let alice2: Vec<_> = User::filter_by_name("alice2").collect(&mut db).await?;
    assert!(alice2.is_empty());

    Ok(())
}

/// When a batch of array creates fails on one element (unique constraint),
/// all prior successful creates are rolled back.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_array_creates_rolls_back_on_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    // Seed the collision
    User::create().name("taken").exec(&mut db).await?;

    t.log().clear();
    assert_err!(
        toasty::batch([
            User::create().name("first"),
            User::create().name("second"),
            User::create().name("taken"), // fails: unique
        ])
        .exec(&mut db)
        .await
    );

    // BEGIN → INSERT first → INSERT second → INSERT taken (fails) → ROLLBACK
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT first
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT second
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // Only the seeded user remains
    let users = User::all().collect::<Vec<_>>(&mut db).await?;
    assert_eq!(1, users.len());
    assert_eq!(users[0].name, "taken");

    Ok(())
}

/// When a batch of different models fails on the second create, the first
/// model's create is rolled back too.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_different_models_rolls_back_on_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        title: String,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    // Seed the collision
    Post::create().title("taken").exec(&mut db).await?;

    t.log().clear();
    assert_err!(
        toasty::batch((
            User::create().name("alice"),
            Post::create().title("taken"), // fails: unique
        ))
        .exec(&mut db)
        .await
    );

    // BEGIN → INSERT user (succeeds) → INSERT post (fails) → ROLLBACK
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT user
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // No user was persisted
    let users = User::all().collect::<Vec<_>>(&mut db).await?;
    assert!(users.is_empty());

    // Only the seeded post remains
    let posts = Post::all().collect::<Vec<_>>(&mut db).await?;
    assert_eq!(1, posts.len());

    Ok(())
}
