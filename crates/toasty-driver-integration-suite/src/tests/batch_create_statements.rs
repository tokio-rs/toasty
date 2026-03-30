use crate::prelude::*;

use toasty_core::{
    driver::{operation::Transaction, Operation},
    stmt::Statement,
};

/// Batch two creates of the same model.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_two_creates_same_model(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

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
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT alice
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT bob
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    // Verify both were persisted
    let all: Vec<_> = User::filter_by_id(alice.id).exec(&mut db).await?;
    assert_eq!(all.len(), 1);
    let all: Vec<_> = User::filter_by_id(bob.id).exec(&mut db).await?;
    assert_eq!(all.len(), 1);

    Ok(())
}

/// Batch creates of two different models.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_two_creates_different_models(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();
    let (user, post): (User, Post) =
        toasty::batch((User::create().name("Alice"), Post::create().title("Hello")))
            .exec(&mut db)
            .await?;

    assert_eq!(user.name, "Alice");
    assert_eq!(post.title, "Hello");

    // Two independent creates → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT user
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT post
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}

/// Batch mixing a query first and a create second.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_query_and_create(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    User::create().name("Alice").exec(&mut db).await?;

    t.log().clear();
    let (users, post): (Vec<User>, Post) =
        toasty::batch((User::filter_by_name("Alice"), Post::create().title("Hello")))
            .exec(&mut db)
            .await?;

    assert_struct!(users, [{ name: "Alice" }]);
    assert_eq!(post.title, "Hello");

    // Two operations (query + create) → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Query(_),
    })); // SELECT
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}

/// Batch mixing a create first and a query second.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_create_then_query(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    User::create().name("Alice").exec(&mut db).await?;

    t.log().clear();
    let (created, existing): (User, Vec<User>) =
        toasty::batch((User::create().name("Bob"), User::filter_by_name("Alice")))
            .exec(&mut db)
            .await?;

    assert_eq!(created.name, "Bob");
    assert_struct!(existing, [{ name: "Alice" }]);

    // Two operations (create + query) → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Query(_),
    })); // SELECT
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}

/// Three-element batch: create, query, create.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_create_query_create(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
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
    assert_struct!(existing, [{ name: "Alice" }]);
    assert_eq!(carol.name, "Carol");

    // Three operations → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT bob
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Query(_),
    })); // SELECT alice
    assert_struct!(t.log().pop_op(), Operation::QuerySql({
        stmt: Statement::Insert(_),
    })); // INSERT carol
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    Ok(())
}

/// Batch creates via an array of create builders.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_creates_from_array(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();
    let users = toasty::batch([
        User::create().name("Alice"),
        User::create().name("Bob"),
        User::create().name("Carol"),
    ])
    .exec(&mut db)
    .await?;

    assert_struct!(users, [{ name: "Alice" }, { name: "Bob" }, { name: "Carol" }]);

    // Three independent creates → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    for _ in 0..3 {
        assert_struct!(t.log().pop_op(), Operation::QuerySql({
            stmt: Statement::Insert(_),
        }));
    }
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    // Verify all were persisted
    for user in &users {
        let found = User::get_by_id(&mut db, user.id).await?;
        assert_eq!(found.name, user.name);
    }

    Ok(())
}

/// Batch creates via a Vec of create builders.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_creates_from_vec(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let names = ["Alice", "Bob", "Carol"];
    let builders: Vec<_> = names.iter().map(|n| User::create().name(*n)).collect();

    t.log().clear();
    let users = toasty::batch(builders).exec(&mut db).await?;

    assert_struct!(users, [{ name: "Alice" }, { name: "Bob" }, { name: "Carol" }]);

    // Three independent creates → transaction-wrapped
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    for _ in 0..3 {
        assert_struct!(t.log().pop_op(), Operation::QuerySql({
            stmt: Statement::Insert(_),
        }));
    }
    assert!(t.log().pop_op().is_transaction_commit());
    assert!(t.log().is_empty());

    // Verify all were persisted
    for user in &users {
        let found = User::get_by_id(&mut db, user.id).await?;
        assert_eq!(found.name, user.name);
    }

    Ok(())
}
