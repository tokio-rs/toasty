//! Driver-level operation assertions for batch creates.
//!
//! Verifies that multi-row batch inserts are wrapped in a transaction
//! (BEGIN … COMMIT) so that partial failures roll back all rows.

use crate::prelude::*;

use toasty::Executor;
use toasty_core::driver::{operation::Transaction, Operation};

/// A batch create of multiple items must be wrapped in a transaction.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_many_wraps_in_transaction(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        title: String,
    }

    let mut db = t.setup_db(models!(Todo)).await;

    t.log().clear();
    let _todos = Todo::create_many()
        .item(Todo::create().title("first"))
        .item(Todo::create().title("second"))
        .exec(&mut db)
        .await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // multi-row INSERT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}

/// A batch create with only one item is a single-row insert and skips the
/// transaction — there is no partial-failure risk with a single row.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_one_item_skips_transaction(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        title: String,
    }

    let mut db = t.setup_db(models!(Todo)).await;

    t.log().clear();
    let _todos = Todo::create_many()
        .item(Todo::create().title("only"))
        .exec(&mut db)
        .await?;

    // Only the INSERT — no transaction wrapping needed for a single row
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    assert!(t.log().is_empty());

    Ok(())
}

/// A batch create with a unique constraint violation must roll back —
/// the driver should see Transaction::Rollback instead of Commit.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_create_unique_violation_rolls_back(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,
    }

    let mut db = t.setup_db(models!(User)).await;

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

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // Only the seeded user remains
    let users = User::all().collect::<Vec<_>>(&mut db).await?;
    assert_eq!(1, users.len());

    Ok(())
}

/// When a batch create runs inside an explicit user transaction, the engine
/// should use savepoints instead of a nested BEGIN/COMMIT.
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

    // The outer transaction start
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

    // Batch inside outer tx uses savepoints
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Savepoint(_))
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // multi-row INSERT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::ReleaseSavepoint(_))
    );

    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}
