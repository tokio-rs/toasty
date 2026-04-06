use crate::prelude::*;

use toasty_core::driver::{Operation, operation::IsolationLevel, operation::Transaction};

// ===== Basic commit / rollback =====

/// Data created inside a committed transaction is visible afterwards.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn commit_persists_data(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// Data created inside a rolled-back transaction is not visible.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn rollback_discards_data(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Ghost").exec(&mut tx).await?;
    tx.rollback().await?;

    let users = User::all().exec(&mut db).await?;
    assert!(users.is_empty());

    Ok(())
}

/// Dropping a transaction without commit or rollback automatically rolls back.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn drop_without_finalize_rolls_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    {
        let mut tx = db.transaction().await?;
        User::create().name("Ghost").exec(&mut tx).await?;
        // tx is dropped here without commit/rollback
    }

    let users = User::all().exec(&mut db).await?;
    assert!(users.is_empty());

    Ok(())
}

/// Multiple operations inside a single transaction are all committed together.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn multiple_ops_in_transaction(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    User::create().name("Bob").exec(&mut tx).await?;
    User::create().name("Carol").exec(&mut tx).await?;
    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 3);

    Ok(())
}

/// Read-your-writes: data created inside a transaction is visible within it
/// before commit.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn read_your_writes(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;

    let users = User::all().exec(&mut tx).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    tx.commit().await?;

    Ok(())
}

/// Updates inside a transaction are committed.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn update_inside_transaction(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;

    let mut tx = db.transaction().await?;
    user.update().name("Bob").exec(&mut tx).await?;
    tx.commit().await?;

    let reloaded = User::get_by_id(&mut db, user.id).await?;
    assert_eq!(reloaded.name, "Bob");

    Ok(())
}

/// Updates inside a rolled-back transaction are discarded.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn update_rolled_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;

    let mut tx = db.transaction().await?;
    user.update().name("Bob").exec(&mut tx).await?;
    tx.rollback().await?;

    let reloaded = User::get_by_id(&mut db, user.id).await?;
    assert_eq!(reloaded.name, "Alice");

    Ok(())
}

/// Deletes inside a rolled-back transaction are discarded.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn delete_rolled_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let user = User::create().name("Alice").exec(&mut db).await?;

    let mut tx = db.transaction().await?;
    User::filter_by_id(user.id).delete().exec(&mut tx).await?;
    tx.rollback().await?;

    let reloaded = User::get_by_id(&mut db, user.id).await?;
    assert_eq!(reloaded.name, "Alice");

    Ok(())
}

// ===== Driver operation log =====

/// Verify the driver receives BEGIN, statements, and COMMIT in the right order.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn driver_sees_begin_commit(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

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
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}

/// Verify the driver receives BEGIN and ROLLBACK when rolled back.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn driver_sees_begin_rollback(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.rollback().await?;

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

    Ok(())
}

// ===== Nested transactions (savepoints) =====

/// A committed nested transaction (savepoint) persists when the outer
/// transaction also commits.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn nested_commit_both(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;

    {
        let mut nested = tx.transaction().await?;
        User::create().name("Bob").exec(&mut nested).await?;
        nested.commit().await?;
    }

    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 2);

    Ok(())
}

/// Rolling back a nested transaction discards only its changes; the outer
/// transaction can still commit its own.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn nested_rollback_inner(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;

    {
        let mut nested = tx.transaction().await?;
        User::create().name("Ghost").exec(&mut nested).await?;
        nested.rollback().await?;
    }

    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// Rolling back the outer transaction discards everything, including changes
/// from an already-committed nested transaction.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn nested_rollback_outer(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;

    {
        let mut nested = tx.transaction().await?;
        User::create().name("Bob").exec(&mut nested).await?;
        nested.commit().await?;
    }

    tx.rollback().await?;

    let users = User::all().exec(&mut db).await?;
    assert!(users.is_empty());

    Ok(())
}

/// Dropping a nested transaction without finalize rolls back just that
/// savepoint.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn nested_drop_rolls_back_savepoint(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;

    {
        let mut nested = tx.transaction().await?;
        User::create().name("Ghost").exec(&mut nested).await?;
        // dropped without commit/rollback
    }

    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// Verify the driver log for a nested transaction shows SAVEPOINT / RELEASE
/// SAVEPOINT around the inner work.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn nested_driver_sees_savepoint_ops(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;

    let mut nested = tx.transaction().await?;
    User::create().name("Bob").exec(&mut nested).await?;
    nested.commit().await?;

    tx.commit().await?;

    // BEGIN
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    // INSERT Alice
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    // SAVEPOINT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Savepoint(_))
    );
    // INSERT Bob
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    // RELEASE SAVEPOINT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::ReleaseSavepoint(_))
    );
    // COMMIT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}

/// Verify the driver log when a nested transaction is rolled back shows
/// ROLLBACK TO SAVEPOINT.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn nested_driver_sees_rollback_to_savepoint(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db.transaction().await?;

    let mut nested = tx.transaction().await?;
    User::create().name("Ghost").exec(&mut nested).await?;
    nested.rollback().await?;

    tx.commit().await?;

    // BEGIN
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    // SAVEPOINT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Savepoint(_))
    );
    // INSERT Ghost
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    // ROLLBACK TO SAVEPOINT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::RollbackToSavepoint(_))
    );
    // COMMIT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}

/// Two sequential nested transactions: first committed, second rolled back.
/// Only data from the first survives.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn two_sequential_nested_transactions(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction().await?;

    {
        let mut nested1 = tx.transaction().await?;
        User::create().name("Alice").exec(&mut nested1).await?;
        nested1.commit().await?;
    }

    {
        let mut nested2 = tx.transaction().await?;
        User::create().name("Ghost").exec(&mut nested2).await?;
        nested2.rollback().await?;
    }

    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

// ===== Statements inside transactions use savepoints for multi-op plans =====

/// When a multi-op statement (e.g. create with association) runs inside an
/// interactive transaction, the engine wraps it in SAVEPOINT/RELEASE instead
/// of BEGIN/COMMIT.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn multi_op_inside_tx_uses_savepoints(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db.transaction().await?;
    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("task"))
        .exec(&mut tx)
        .await?;
    tx.commit().await?;

    // BEGIN (interactive tx)
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false
        })
    );
    // SAVEPOINT (engine wraps the multi-op plan)
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Savepoint(_))
    );
    // INSERT user
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    // INSERT todo
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    // RELEASE SAVEPOINT
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::ReleaseSavepoint(_))
    );
    // COMMIT (interactive tx)
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    // Verify the data landed
    let todos = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "task");

    Ok(())
}

// ===== TransactionBuilder API =====

/// TransactionBuilder from Db commits data like a regular transaction.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn builder_on_db_commit(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut tx = db.transaction_builder().begin().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// TransactionBuilder from Connection commits data like a regular transaction.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn builder_on_connection_commit(t: &mut Test) -> Result<()> {
    let db = setup(t).await;
    let mut conn = db.connection().await?;

    let mut tx = conn.transaction_builder().begin().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

    let users = User::all().exec(&mut conn).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// TransactionBuilder with isolation level sends the correct option to the driver.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn builder_with_isolation_level(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .begin()
        .await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: Some(IsolationLevel::Serializable),
            read_only: false
        })
    );

    Ok(())
}

/// TransactionBuilder with read_only sends the correct option to the driver.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn builder_with_read_only(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let tx = db.transaction_builder().read_only(true).begin().await?;
    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: true
        })
    );

    Ok(())
}

/// TransactionBuilder with both isolation and read_only sends both options.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn builder_with_all_options(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let tx = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .read_only(true)
        .begin()
        .await?;
    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: Some(IsolationLevel::Serializable),
            read_only: true
        })
    );

    Ok(())
}

/// TransactionBuilder auto-rolls back on drop just like a regular transaction.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn builder_drop_rolls_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    {
        let mut tx = db.transaction_builder().begin().await?;
        User::create().name("Ghost").exec(&mut tx).await?;
    }

    let users = User::all().exec(&mut db).await?;
    assert!(users.is_empty());

    Ok(())
}

/// Calling `.transaction()` through `&mut dyn Executor` works.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn transaction_via_dyn_executor(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let executor: &mut dyn toasty::Executor = &mut db;
    let mut tx = executor.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}
