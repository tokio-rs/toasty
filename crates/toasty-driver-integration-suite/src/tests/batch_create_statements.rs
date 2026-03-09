use crate::prelude::*;

use toasty_core::driver::{operation::Transaction, Operation};

/// Batch two creates of the same model.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_creates_same_model(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    t.log().clear();
    let (alice, bob): (User, User) =
        toasty::batch((User::create().name("Alice"), User::create().name("Bob")))
            .exec(&mut db)
            .await?;

    assert_eq!(alice.name, "Alice");
    assert_eq!(bob.name, "Bob");

    // Two independent creates → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert!(t.log().pop_op().is_query_sql()); // INSERT alice
    assert!(t.log().pop_op().is_query_sql()); // INSERT bob
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    // Verify both were persisted
    let all: Vec<_> = User::filter_by_id(alice.id).collect(&mut db).await?;
    assert_eq!(all.len(), 1);
    let all: Vec<_> = User::filter_by_id(bob.id).collect(&mut db).await?;
    assert_eq!(all.len(), 1);

    Ok(())
}

/// Batch creates of two different models.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_creates_different_models(t: &mut Test) -> Result<()> {
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
        title: String,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    t.log().clear();
    let (user, post): (User, Post) = toasty::batch((
        User::create().name("Alice"),
        Post::create().title("Hello World"),
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Alice");
    assert_eq!(post.title, "Hello World");

    // Two independent creates → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert!(t.log().pop_op().is_query_sql()); // INSERT user
    assert!(t.log().pop_op().is_query_sql()); // INSERT post
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    // Verify persistence
    let found = User::get_by_id(&mut db, user.id).await?;
    assert_eq!(found.name, "Alice");
    let found = Post::get_by_id(&mut db, post.id).await?;
    assert_eq!(found.title, "Hello World");

    Ok(())
}

/// Batch mixing a create and a query, with the create coming second.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_query_then_create(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    User::create().name("Alice").exec(&mut db).await?;

    t.log().clear();
    let (existing, created): (Vec<User>, User) =
        toasty::batch((User::filter_by_name("Alice"), User::create().name("Bob")))
            .exec(&mut db)
            .await?;

    assert_struct!(existing, [_ { name: "Alice" }]);
    assert_eq!(created.name, "Bob");

    // Two operations (query + create) → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert!(t.log().pop_op().is_query_sql()); // SELECT
    assert!(t.log().pop_op().is_query_sql()); // INSERT
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}

/// Batch mixing a create first and a query second.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_then_query(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    User::create().name("Alice").exec(&mut db).await?;

    t.log().clear();
    let (created, existing): (User, Vec<User>) =
        toasty::batch((User::create().name("Bob"), User::filter_by_name("Alice")))
            .exec(&mut db)
            .await?;

    assert_eq!(created.name, "Bob");
    assert_struct!(existing, [_ { name: "Alice" }]);

    // Two operations (create + query) → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert!(t.log().pop_op().is_query_sql()); // INSERT
    assert!(t.log().pop_op().is_query_sql()); // SELECT
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}

/// Three-element batch: create, query, create.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_query_create(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    User::create().name("Alice").exec(&mut db).await?;

    t.log().clear();
    let (bob, existing, carol): (User, Vec<User>, User) = toasty::batch((
        User::create().name("Bob"),
        User::filter_by_name("Alice"),
        User::create().name("Carol"),
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(bob.name, "Bob");
    assert_struct!(existing, [_ { name: "Alice" }]);
    assert_eq!(carol.name, "Carol");

    // Three operations → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert!(t.log().pop_op().is_query_sql()); // INSERT bob
    assert!(t.log().pop_op().is_query_sql()); // SELECT alice
    assert!(t.log().pop_op().is_query_sql()); // INSERT carol
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}
