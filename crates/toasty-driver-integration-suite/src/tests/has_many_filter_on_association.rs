//! Test filtering parent models by conditions on has_many associations

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn filter_parent_by_child_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

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

        complete: bool,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create users
    let alice = User::create().name("Alice").exec(&mut db).await?;
    let bob = User::create().name("Bob").exec(&mut db).await?;
    let carol = User::create().name("Carol").exec(&mut db).await?;

    // Alice has one incomplete todo
    alice
        .todos()
        .create()
        .title("buy groceries")
        .complete(false)
        .exec(&mut db)
        .await?;

    // Bob has only completed todos
    bob.todos()
        .create()
        .title("read book")
        .complete(true)
        .exec(&mut db)
        .await?;

    // Carol has both complete and incomplete todos
    carol
        .todos()
        .create()
        .title("clean house")
        .complete(true)
        .exec(&mut db)
        .await?;
    carol
        .todos()
        .create()
        .title("write report")
        .complete(false)
        .exec(&mut db)
        .await?;

    // Find users who have at least one incomplete todo
    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .any(Todo::fields().complete().eq(false)),
    )
    .collect(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Alice", "Carol"]);

    // Find users who have at least one completed todo
    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .any(Todo::fields().complete().eq(true)),
    )
    .collect(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Bob", "Carol"]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn filter_parent_no_matching_children(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

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

        priority: i64,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().name("Alice").exec(&mut db).await?;
    user.todos()
        .create()
        .title("low priority")
        .priority(1)
        .exec(&mut db)
        .await?;

    // No todos with priority > 5 exist
    let users: Vec<_> = User::filter(User::fields().todos().any(Todo::fields().priority().gt(5)))
        .collect(&mut db)
        .await?;

    assert!(users.is_empty());

    Ok(())
}
