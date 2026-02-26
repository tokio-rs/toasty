use crate::prelude::*;

use toasty_core::driver::{operation::Transaction, Operation};

// ===== Transaction wrapping =====

/// A multi-op create (user + associated todo) should be wrapped in
/// BEGIN ... COMMIT so the driver sees all three transaction operations.
#[driver_test(id(ID), requires(sql))]
pub async fn multi_op_create_wraps_in_transaction(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    t.log().clear();
    let user = User::create()
        .todo(Todo::create().title("task"))
        .exec(&db)
        .await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start { isolation: None })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT user
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT todo
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    let todos = user.todos().collect::<Vec<_>>(&db).await?;
    assert_eq!(1, todos.len());

    Ok(())
}

/// A single-op create (no associations) must NOT be wrapped in a transaction —
/// the engine skips the overhead for plans with only one DB operation.
#[driver_test(id(ID), requires(sql))]
pub async fn single_op_skips_transaction(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
    }

    let db = t.setup_db(models!(User)).await;

    t.log().clear();
    User::create().exec(&db).await?;

    // Only the INSERT — no Transaction::Start bookending it
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    assert!(t.log().is_empty());

    Ok(())
}

// ===== Rollback on partial failure =====

/// When the second INSERT in a has_many create plan fails (unique constraint),
/// the driver should receive Transaction::Rollback and no orphaned user should
/// remain in the database.
///
/// Uses u64 (auto-increment) IDs so that the engine always generates two
/// separate DB operations (INSERT user then INSERT todo), ensuring the
/// explicit transaction wrapping is exercised. With uuid::Uuid IDs the engine
/// reorders execution (INSERT todo before INSERT user due to the Const
/// optimization), which produces a different but equally valid log pattern.
#[driver_test(requires(sql))]
pub async fn create_with_has_many_rolls_back_on_failure(t: &mut Test) -> Result<()> {
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

        #[unique]
        title: String,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    // Seed the title that will cause the second INSERT to fail.
    User::create()
        .todo(Todo::create().title("taken"))
        .exec(&db)
        .await?;

    t.log().clear();
    assert_err!(
        User::create()
            .todo(Todo::create().title("taken"))
            .exec(&db)
            .await
    );

    // Transaction::Start → INSERT user (succeeds, logged) →
    // INSERT todo (fails on unique constraint, NOT logged) → Transaction::Rollback
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start { isolation: None })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT user
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // No orphaned user — count unchanged from pre-seed
    let users = User::all().collect::<Vec<_>>(&db).await?;
    assert_eq!(1, users.len());

    Ok(())
}

/// Same rollback guarantee for a has_one association create.
///
/// Uses u64 (auto-increment) IDs so that the engine always generates two
/// separate DB operations (INSERT user then INSERT profile), ensuring the
/// explicit transaction wrapping is exercised. With uuid::Uuid IDs the engine
/// can combine both inserts into a single atomic SQL statement, which provides
/// atomicity without an explicit transaction.
#[driver_test(requires(sql))]
pub async fn create_with_has_one_rolls_back_on_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: u64,

        #[unique]
        bio: String,

        #[unique]
        user_id: u64,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = t.setup_db(models!(User, Profile)).await;

    // Seed the bio that will cause the second INSERT to fail.
    User::create()
        .profile(Profile::create().bio("taken"))
        .exec(&db)
        .await?;

    t.log().clear();
    assert_err!(
        User::create()
            .profile(Profile::create().bio("taken"))
            .exec(&db)
            .await
    );

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start { isolation: None })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT user
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // No orphaned user — count unchanged from pre-seed
    let users = User::all().collect::<Vec<_>>(&db).await?;
    assert_eq!(1, users.len());

    Ok(())
}

/// When an update + new-association plan fails on the UPDATE (after the
/// INSERT succeeds), the INSERT must also be rolled back.
///
/// The engine always executes INSERT before UPDATE in such plans (INSERT is
/// a dependency of the UPDATE's returning clause). So the collision is placed
/// on the User's name field (not the Todo), ensuring the INSERT succeeds first
/// and is then rolled back when the subsequent UPDATE fails.
#[driver_test(id(ID), requires(sql))]
pub async fn update_with_new_association_rolls_back_on_failure(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    let mut user = User::create().name("original").exec(&db).await?;
    // Seed the name collision — this user's name will be duplicated by the failing UPDATE.
    User::create().name("taken").exec(&db).await?;

    t.log().clear();
    assert_err!(
        user.update()
            .name("taken") // UPDATE will fail: unique name
            .todo(Todo::create().title("new-todo")) // INSERT runs first and succeeds
            .exec(&db)
            .await
    );

    // INSERT todo runs first (succeeds, logged), then UPDATE user fails on unique
    // name → Transaction::Rollback undoes the INSERT.
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start { isolation: None })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // INSERT todo (rolled back)
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Rollback)
    );
    assert!(t.log().is_empty());

    // INSERT was rolled back — no orphaned todo
    let todos = user.todos().collect::<Vec<_>>(&db).await?;
    assert!(todos.is_empty());

    Ok(())
}

// ===== ReadModifyWrite transaction behavior =====

/// A successful standalone conditional update (link/unlink) wraps itself in
/// its own BEGIN...COMMIT on drivers that don't support CTE-with-update
/// (SQLite, MySQL). When nested inside an outer transaction it uses savepoints
/// instead. On PostgreSQL the same operation is a single CTE-based QuerySql.
#[driver_test(id(ID), requires(sql))]
pub async fn rmw_uses_savepoints(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    let user = User::create().todo(Todo::create()).exec(&db).await?;
    let todos: Vec<_> = user.todos().collect(&db).await?;

    t.log().clear();
    user.todos().remove(&db, &todos[0]).await?;

    if t.capability().cte_with_update {
        // PostgreSQL: single CTE bundles the condition + update
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    } else {
        // SQLite / MySQL: standalone RMW starts its own transaction
        assert_struct!(
            t.log().pop_op(),
            Operation::Transaction(Transaction::Start { isolation: None })
        );
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // read
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // write
        assert_struct!(
            t.log().pop_op(),
            Operation::Transaction(Transaction::Commit)
        );
    }
    assert!(t.log().is_empty());

    Ok(())
}

/// When a standalone RMW condition fails (todo doesn't belong to this user),
/// the driver should receive ROLLBACK on the RMW's own transaction.
/// On PostgreSQL the CTE handles this in a single statement.
#[driver_test(id(ID), requires(sql))]
pub async fn rmw_condition_failure_issues_rollback_to_savepoint(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = t.setup_db(models!(User, Todo)).await;

    let user1 = User::create().exec(&db).await?;
    let user2 = User::create().todo(Todo::create()).exec(&db).await?;
    let u2_todos: Vec<_> = user2.todos().collect(&db).await?;

    t.log().clear();
    // Remove u2's todo via user1 — condition (user_id = user1.id) won't match
    assert_err!(user1.todos().remove(&db, &u2_todos[0]).await);

    if t.capability().cte_with_update {
        // PostgreSQL: a single QuerySql; condition handled inside the CTE
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    } else {
        // SQLite / MySQL: standalone RMW starts its own transaction;
        // condition failure rolls it back
        assert_struct!(
            t.log().pop_op(),
            Operation::Transaction(Transaction::Start { isolation: None })
        );
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_)); // read
        assert_struct!(
            t.log().pop_op(),
            Operation::Transaction(Transaction::Rollback)
        );
    }
    assert!(t.log().is_empty());

    // The todo is untouched — still belongs to user2
    let reloaded = Todo::get_by_id(&db, u2_todos[0].id).await?;
    assert_struct!(reloaded, _ { user_id: Some(== user2.id), .. });

    Ok(())
}
