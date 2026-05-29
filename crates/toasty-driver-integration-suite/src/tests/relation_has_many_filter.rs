//! Test filtering parent models by conditions on has_many associations

use crate::prelude::*;

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::has_many_belongs_to_with_flags)
)]
pub async fn filter_parent_by_child_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

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
        .priority(0)
        .exec(&mut db)
        .await?;

    // Bob has only completed todos
    bob.todos()
        .create()
        .title("read book")
        .complete(true)
        .priority(0)
        .exec(&mut db)
        .await?;

    // Carol has both complete and incomplete todos
    carol
        .todos()
        .create()
        .title("clean house")
        .complete(true)
        .priority(0)
        .exec(&mut db)
        .await?;
    carol
        .todos()
        .create()
        .title("write report")
        .complete(false)
        .priority(0)
        .exec(&mut db)
        .await?;

    // Find users who have at least one incomplete todo
    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .any(Todo::fields().complete().eq(false)),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Alice", "Carol"]);

    // Find users who have at least one completed todo
    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .any(Todo::fields().complete().eq(true)),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Bob", "Carol"]);

    Ok(())
}

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::has_many_belongs_to_with_flags)
)]
pub async fn filter_parent_no_matching_children(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().name("Alice").exec(&mut db).await?;
    user.todos()
        .create()
        .title("low priority")
        .complete(false)
        .priority(1)
        .exec(&mut db)
        .await?;

    // No todos with priority > 5 exist
    let users: Vec<_> = User::filter(User::fields().todos().any(Todo::fields().priority().gt(5)))
        .exec(&mut db)
        .await?;

    assert!(users.is_empty());

    Ok(())
}

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::has_many_belongs_to)
)]
pub async fn filter_parent_all_children_match(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        { name: "Alice", todos: [{ title: "urgent" }] },
        { name: "Bob", todos: [{ title: "later" }, { title: "later" }] },
        { name: "Carol", todos: [{ title: "urgent" }, { title: "later" }] },
        // Dan has no todos — should match `all(...)` vacuously.
        { name: "Dan" },
    ])
    .exec(&mut db)
    .await?;

    // All todos "urgent" → Alice (only urgent) and Dan (no todos, vacuous).
    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .all(Todo::fields().title().eq("urgent")),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Alice", "Dan"]);

    // All todos "later" → Bob and Dan.
    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .all(Todo::fields().title().eq("later")),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Bob", "Dan"]);

    Ok(())
}
