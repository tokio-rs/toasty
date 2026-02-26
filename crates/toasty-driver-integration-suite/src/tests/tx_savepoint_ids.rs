use crate::prelude::*;

use toasty_core::driver::{operation::Transaction, Operation};

/// A multi-op statement executed inside an interactive transaction should use
/// savepoints (not BEGIN/COMMIT) with IDs that don't collide with any
/// savepoints created by the interactive transaction layer.
#[driver_test(requires(sql))]
pub async fn multi_op_inside_transaction(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,

        #[index]
        user_id: u64,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    db.transaction(async |tx| {
        t.log().clear();

        // Multi-op: INSERT user + INSERT todo → exec-plan wraps in savepoint
        User::create()
            .todo(Todo::create().title("task"))
            .exec(tx)
            .await?;

        // Inside a transaction, exec-plan should use Savepoint/ReleaseSavepoint
        // instead of Start/Commit. The savepoint ID must be 0 since no prior
        // savepoints exist on this connection.
        assert_struct!(
            t.log().pop_op(),
            Operation::Transaction(Transaction::Savepoint(0))
        );
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT user
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT todo
        assert_struct!(
            t.log().pop_op(),
            Operation::Transaction(Transaction::ReleaseSavepoint(0))
        );
        assert!(t.log().is_empty());

        Ok::<(), toasty::Error>(())
    })
    .await?;

    let users = User::all().collect::<Vec<_>>(&db).await?;
    assert_eq!(1, users.len());
    Ok(())
}

/// A multi-op statement inside a nested interactive transaction (savepoint)
/// must use savepoint IDs that don't collide with the nesting savepoint.
///
/// The interactive `tx.transaction()` creates sp_0, so the exec-plan's
/// savepoint must use sp_1 (or higher).
#[driver_test(requires(sql))]
pub async fn multi_op_inside_nested_transaction(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,

        #[index]
        user_id: u64,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    db.transaction(async |tx| {
        tx.transaction(async |inner| {
            t.log().clear();

            // Multi-op inside a nested transaction. The nested tx already
            // consumed savepoint ID 0 (sp_0), so the exec-plan must use
            // ID 1 (sp_1).
            User::create()
                .todo(Todo::create().title("task"))
                .exec(inner)
                .await?;

            assert_struct!(
                t.log().pop_op(),
                Operation::Transaction(Transaction::Savepoint(1))
            );
            assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT user
            assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT todo
            assert_struct!(
                t.log().pop_op(),
                Operation::Transaction(Transaction::ReleaseSavepoint(1))
            );
            assert!(t.log().is_empty());

            Ok::<(), toasty::Error>(())
        })
        .await?;

        Ok::<(), toasty::Error>(())
    })
    .await?;

    let users = User::all().collect::<Vec<_>>(&db).await?;
    assert_eq!(1, users.len());
    Ok(())
}
