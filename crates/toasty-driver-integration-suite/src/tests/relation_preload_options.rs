use crate::prelude::*;

/// Ordering an include applies independently to each parent's loaded relation.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_ordered_per_parent(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        {
            name: "alice",
            todos: [{ title: "a" }, { title: "c" }, { title: "b" }]
        },
        {
            name: "bob",
            todos: [{ title: "x" }, { title: "z" }, { title: "y" }]
        },
    ])
    .exec(&mut db)
    .await?;

    let users: Vec<User> = User::all()
        .order_by(User::fields().name().asc())
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().title().desc()),
        )
        .exec(&mut db)
        .await?;

    assert_struct!(users, [
        { name: "alice", todos.get(): [{ title: "c" }, { title: "b" }, { title: "a" }] },
        { name: "bob", todos.get(): [{ title: "z" }, { title: "y" }, { title: "x" }] },
    ]);

    Ok(())
}

/// Include filters run before ordering.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_filter_and_order(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "c" }, { title: "b" }]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().ne("c"))
                .order_by(Todo::fields().title().desc()),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.todos.get(), #(
        { title: "b" },
        { title: "a" },
    ));

    Ok(())
}

/// Later explicit ordering replaces earlier ordering for the same path.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_repeated_ordering_last_wins(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "c" }, { title: "b" }]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().title().asc()),
        )
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().title().desc()),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.todos.get(), #(
        { title: "c" },
        { title: "b" },
        { title: "a" },
    ));

    Ok(())
}

/// Include ordering is rejected when the terminal relation is singular.
#[driver_test(id(ID), scenario(crate::scenarios::has_one_optional_belongs_to))]
pub async fn preload_has_one_ordering_rejected(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let err = assert_err!(
        User::all()
            .include(
                User::fields()
                    .profile()
                    .order_by(Profile::fields().bio().asc()),
            )
            .exec(&mut db)
            .await
    );

    assert!(err.is_invalid_statement(), "unexpected error: {err}");

    Ok(())
}
