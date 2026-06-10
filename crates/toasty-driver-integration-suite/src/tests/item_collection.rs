//! Tests for the item collection feature: multiple models sharing one DynamoDB table
//! via a composite sort key synthesized from the partition key + an auto-minted
//! local-id segment owned by Toasty.
//!
//! In v3, a child model declares its parent with a field-level `#[item_parent]`
//! attribute on a `Deferred<Parent>` field. Both parent and child carry the same
//! `#[key(account, sk)]` shape; Toasty owns the contents of `sk`.

use crate::prelude::*;

// ---------------------------------------------------------------------------
// Shared model definitions used across tests in this file
// ---------------------------------------------------------------------------

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct User {
    account: String,

    #[auto]
    sk: String,

    name: String,

    #[has_many]
    todos: toasty::Deferred<Vec<Todo>>,
}

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct Todo {
    account: String,

    #[auto]
    sk: String,

    title: String,

    #[item_parent]
    user: toasty::Deferred<User>,
}

async fn setup(test: &mut Test) -> toasty::Db {
    test.setup_db(models!(User, Todo)).await
}

// ---------------------------------------------------------------------------
// CRUD tests — require a real database with native_starts_with support
// ---------------------------------------------------------------------------

/// Basic create / read / delete cycle for a user + their todos sharing one table.
#[driver_test(requires(not(sql)))]
pub async fn crud_create_read_delete(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    // New user has no todos
    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    // Create a todo through the user relation
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    assert_eq!(todo.account, user.account);

    // Reload by full composite key
    let reloaded = Todo::get_by_account_and_sk(&mut db, &todo.account, &todo.sk).await?;
    assert_eq!(reloaded.title, "hello world");

    // User scope still shows exactly one todo
    let list = user.todos().exec(&mut db).await?;
    assert_eq!(1, list.len());

    // Delete the todo
    todo.delete().exec(&mut db).await?;

    assert_err!(Todo::get_by_account_and_sk(&mut db, &reloaded.account, &reloaded.sk).await);
    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    Ok(())
}

/// Todos created for user A must not appear in user B's scope.
#[driver_test(requires(not(sql)))]
#[ignore = "deferred: IC root sk auto-mint produces colliding `<Model>#` for sibling roots; resolved by C5 (#[local_id] for roots)"]
pub async fn scoped_isolation(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user_a = toasty::create!(User {
        account: "acct",
        name: "a"
    })
    .exec(&mut db)
    .await?;
    let user_b = toasty::create!(User {
        account: "acct",
        name: "b"
    })
    .exec(&mut db)
    .await?;

    let todo_a = user_a
        .todos()
        .create()
        .title("user A todo")
        .exec(&mut db)
        .await?;

    // user_b sees no todos
    assert_eq!(0, user_b.todos().exec(&mut db).await?.len());

    // user_a's todo is not visible via user_b scope
    assert_none!(
        user_b
            .todos()
            .filter_by_sk(todo_a.sk.clone())
            .first()
            .exec(&mut db)
            .await?
    );

    Ok(())
}

/// Multiple todos under the same user can all be loaded, updated, and deleted.
#[driver_test(requires(not(sql)))]
pub async fn multiple_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    for i in 0..5 {
        user.todos()
            .create()
            .title(format!("todo {i}"))
            .exec(&mut db)
            .await?;
    }

    let list = user.todos().exec(&mut db).await?;
    assert_eq!(5, list.len());

    Ok(())
}

/// Todos can be included on the parent.
#[driver_test(requires(not(sql)))]
pub async fn include_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    for i in 0..5 {
        user.todos()
            .create()
            .title(format!("todo {i}"))
            .exec(&mut db)
            .await?;
    }

    let mut users = User::filter_by_account(&user.account)
        .filter_by_sk(user.sk.clone())
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;
    assert_eq!(1, users.len());
    let user = users.pop().unwrap();
    assert_eq!(5, user.todos.get().len());

    Ok(())
}

/// Scoped filter_by_sk returns the right todo when the user matches and
/// nothing when the user does not match.
#[driver_test(requires(not(sql)))]
#[ignore = "deferred: IC root sk auto-mint produces colliding `<Model>#` for sibling roots; resolved by C5 (#[local_id] for roots)"]
pub async fn scoped_filter_by_id(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user1 = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;
    let user2 = toasty::create!(User {
        account: "acct",
        name: "u2"
    })
    .exec(&mut db)
    .await?;

    let todo = user1.todos().create().title("hello").exec(&mut db).await?;

    // Correct scope finds the todo
    let found = user1
        .todos()
        .filter_by_sk(todo.sk.clone())
        .first()
        .exec(&mut db)
        .await?
        .expect("todo should be visible in its parent's scope");
    assert_eq!(found.sk, todo.sk);

    // Wrong scope does not find it
    assert_none!(
        user2
            .todos()
            .filter_by_sk(todo.sk.clone())
            .first()
            .exec(&mut db)
            .await?
    );

    Ok(())
}

/// Scoped update modifies only the target todo and leaves others unchanged.
#[driver_test(requires(not(sql)))]
pub async fn scoped_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    let todo = user
        .todos()
        .create()
        .title("original")
        .exec(&mut db)
        .await?;

    user.todos()
        .filter_by_sk(todo.sk.clone())
        .update()
        .title("updated")
        .exec(&mut db)
        .await?;

    let reloaded = Todo::get_by_account_and_sk(&mut db, &todo.account, &todo.sk).await?;
    assert_eq!(reloaded.title, "updated");

    Ok(())
}

/// Deleting a user via its own scope does not silently fail.  After deletion
/// the user and all their todos are gone.
#[driver_test(requires(not(sql)))]
pub async fn delete_user_removes_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;
    let todo = user
        .todos()
        .create()
        .title("should disappear")
        .exec(&mut db)
        .await?;

    let user_account = user.account.clone();
    let user_sk = user.sk.clone();
    let todo_account = todo.account.clone();
    let todo_sk = todo.sk.clone();

    user.delete().exec(&mut db).await?;

    assert_err!(User::get_by_account_and_sk(&mut db, &user_account, &user_sk).await);
    assert_err!(Todo::get_by_account_and_sk(&mut db, &todo_account, &todo_sk).await);

    Ok(())
}

/// `parent` field on the app schema model is set to the parent model's id for
/// the child, and remains unset for the root.
#[driver_test]
pub async fn schema_records_item_parent(test: &mut Test) {
    use toasty::schema::Model;

    let _db = setup(test).await;

    let todo_parent = <Todo as Model>::schema().as_root_unwrap().parent;
    assert_eq!(todo_parent, Some(<User as Model>::id()));

    let user_parent = <User as Model>::schema().as_root_unwrap().parent;
    assert_eq!(user_parent, None);
}

// ---------------------------------------------------------------------------
// Filter-bearing operations on item-collection child models
//
// These exercise paths where the discriminator predicate (Expr::IsModel)
// reaches the driver's general expression serializer rather than the
// primary-key serializer:
//
//   - Scan filter:           query with no PK constraint, just a non-key filter.
//   - DeleteByKey filter:    delete with both a PK identity and a row filter.
//   - UpdateByKey filter:    update with both a PK identity and a row filter.
//
// On DynamoDB these flow through `ddb_expression`. On other DDB-shaped paths
// (BuildKeyExpression) the discriminator already gets folded into the SK
// prefix; these tests cover the second serializer.
// ---------------------------------------------------------------------------

/// Scan a child model with a filter on a non-key field. The lowering pipeline
/// adds an `IsModel(Todo)` conjunct alongside the user filter; the planner
/// can't promote it to a key condition (no PK column referenced), so it
/// flows into the scan's filter expression and through `ddb_expression`.
#[driver_test(requires(not(sql)), requires(not(sql)))]
pub async fn scan_child_model_with_non_key_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;
    user.todos()
        .create()
        .title("match me")
        .exec(&mut db)
        .await?;
    user.todos().create().title("skip me").exec(&mut db).await?;

    let results: Vec<Todo> = Todo::filter(Todo::fields().title().eq("match me"))
        .exec(&mut db)
        .await?;

    assert_eq!(1, results.len());
    assert_eq!("match me", results[0].title);

    Ok(())
}

/// Delete a child model row with a filter on a non-key field. The driver
/// receives a `DeleteByKey` op whose filter contains `IsModel(Todo) AND
/// title = "..."` — `ddb_expression` serializes the filter as a DynamoDB
/// condition expression.
#[driver_test(requires(not(sql)))]
pub async fn delete_child_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;
    let target = user
        .todos()
        .create()
        .title("delete me")
        .exec(&mut db)
        .await?;
    // Sibling under the same user. The filter must narrow to `target`
    // alone — otherwise this row would also disappear.
    let sibling = user.todos().create().title("keep me").exec(&mut db).await?;

    user.todos()
        .filter_by_sk(target.sk.clone())
        .filter(Todo::fields().title().eq("delete me"))
        .delete()
        .exec(&mut db)
        .await?;

    assert_err!(Todo::get_by_account_and_sk(&mut db, &target.account, &target.sk).await);
    let survivor = Todo::get_by_account_and_sk(&mut db, &sibling.account, &sibling.sk).await?;
    assert_eq!(survivor.title, "keep me");

    Ok(())
}

/// When the filter does not match, the delete is a no-op: the row remains
/// in place. Proves the filter is consulted, not silently dropped.
#[driver_test(requires(not(sql)))]
pub async fn delete_child_with_non_matching_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;
    let todo = user
        .todos()
        .create()
        .title("actual title")
        .exec(&mut db)
        .await?;

    user.todos()
        .filter_by_sk(todo.sk.clone())
        .filter(Todo::fields().title().eq("wrong title"))
        .delete()
        .exec(&mut db)
        .await?;

    let survivor = Todo::get_by_account_and_sk(&mut db, &todo.account, &todo.sk).await?;
    assert_eq!(survivor.title, "actual title");

    Ok(())
}

/// Update a child model row with a filter on a non-key field. The driver
/// receives an `UpdateByKey` op whose filter contains `IsModel(Todo) AND
/// title = "..."` — `ddb_expression` serializes it.
#[driver_test(requires(not(sql)))]
pub async fn update_child_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;
    let todo = user.todos().create().title("before").exec(&mut db).await?;

    user.todos()
        .filter_by_sk(todo.sk.clone())
        .filter(Todo::fields().title().eq("before"))
        .update()
        .title("after")
        .exec(&mut db)
        .await?;

    let reloaded = Todo::get_by_account_and_sk(&mut db, &todo.account, &todo.sk).await?;
    assert_eq!(reloaded.title, "after");

    Ok(())
}

/// Filter-driven update across multiple child rows. The filter matches
/// more than one Todo; all matching rows must be updated, non-matching
/// rows must remain unchanged. Distinct from `update_child_with_filter`,
/// which targets a single row by composite PK.
#[driver_test(requires(not(sql)))]
pub async fn update_many_children_by_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    let mut todos = vec![];
    for label in ["match", "match", "match", "skip"] {
        todos.push(
            user.todos()
                .create()
                .title(label.to_string())
                .exec(&mut db)
                .await?,
        );
    }

    user.todos()
        .filter(Todo::fields().title().eq("match"))
        .update()
        .title("updated")
        .exec(&mut db)
        .await?;

    let mut titles: Vec<String> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["skip", "updated", "updated", "updated"],);

    Ok(())
}

/// Filter-driven delete across multiple child rows. The filter matches
/// more than one Todo; all matching rows must be deleted, non-matching
/// rows must survive. Distinct from `delete_child_with_filter`, which
/// targets a single row by composite PK.
#[driver_test(requires(not(sql)))]
pub async fn delete_many_children_by_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    for label in ["doomed", "doomed", "doomed", "survivor"] {
        user.todos()
            .create()
            .title(label.to_string())
            .exec(&mut db)
            .await?;
    }

    user.todos()
        .filter(Todo::fields().title().eq("doomed"))
        .delete()
        .exec(&mut db)
        .await?;

    let titles: Vec<String> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, vec!["survivor".to_string()]);

    Ok(())
}

/// Batch-create multiple child rows under one parent. Exercises
/// `Insert::merge` for `InsertTarget::Scope` targets — the planner builds
/// one INSERT per row and merges them into a single VALUES list before
/// dispatching to the driver. All rows must round-trip and remain
/// addressable by the user's scope.
#[driver_test(requires(not(sql)))]
pub async fn batch_create_children(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        account: "acct",
        name: "u1"
    })
    .exec(&mut db)
    .await?;

    let mut builder = Todo::create_many();
    for i in 0..5 {
        builder = builder.item(user.todos().create().title(format!("todo {i}")));
    }
    let todos = builder.exec(&mut db).await?;
    assert_eq!(5, todos.len());

    let mut titles: Vec<String> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(
        titles,
        vec!["todo 0", "todo 1", "todo 2", "todo 3", "todo 4"]
    );

    Ok(())
}
