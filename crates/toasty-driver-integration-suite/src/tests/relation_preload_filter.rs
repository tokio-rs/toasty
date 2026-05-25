use crate::prelude::*;

/// Filtering a `HasMany` include loads only matching children. Parents
/// with no matching children are returned with an empty preloaded `Vec`.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "b" }, { title: "c" }]
    })
    .exec(&mut db)
    .await?;

    // Bob has no matching todos.
    let bob = toasty::create!(User {
        name: "bob",
        todos: [{ title: "x" }]
    })
    .exec(&mut db)
    .await?;

    let alice_loaded = User::filter_by_id(alice.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("b")),
        )
        .get(&mut db)
        .await?;

    let titles: Vec<&str> = alice_loaded
        .todos
        .get()
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, vec!["b"]);

    let bob_loaded = User::filter_by_id(bob.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("b")),
        )
        .get(&mut db)
        .await?;
    assert!(bob_loaded.todos.get().is_empty());

    Ok(())
}

/// An include filter is independent of a parent-side `.any(...)`. The
/// parent filter selects which users come back; the include filter
/// selects which todos travel with each user.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_filter_independent_of_parent_any(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let _alice = toasty::create!(User {
        name: "alice",
        todos: [{ title: "keep" }, { title: "drop" }]
    })
    .exec(&mut db)
    .await?;

    // Bob has no `keep` todo, so the parent-side `any` excludes him.
    let _bob = toasty::create!(User {
        name: "bob",
        todos: [{ title: "drop" }]
    })
    .exec(&mut db)
    .await?;

    let users: Vec<User> = User::all()
        .filter(
            User::fields()
                .todos()
                .any(Todo::fields().title().eq("keep")),
        )
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("keep")),
        )
        .exec(&mut db)
        .await?;

    assert_eq!(1, users.len());
    let alice = &users[0];
    assert_eq!("alice", alice.name);
    let titles: Vec<&str> = alice.todos.get().iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["keep"]);

    Ok(())
}

/// Multiple `.include(...)` calls on the same relation with different
/// filters AND the predicates together.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_repeated_filters_anded(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User {
        name: "alice",
        todos: [{ title: "aaa" }, { title: "aab" }, { title: "bbb" }]
    })
    .exec(&mut db)
    .await?;

    let alice_loaded = User::filter_by_id(alice.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("aab")),
        )
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("aab")),
        )
        .get(&mut db)
        .await?;

    let titles: Vec<&str> = alice_loaded
        .todos
        .get()
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, vec!["aab"]);

    Ok(())
}

/// Filtering a `HasOne` include: when the predicate excludes the row,
/// the relation loads as `None`; when it matches, as `Some`.
#[driver_test(id(ID), scenario(crate::scenarios::has_one_optional_belongs_to))]
pub async fn preload_has_one_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        profile: { bio: "public bio" }
    })
    .exec(&mut db)
    .await?;

    // Filter excludes the row → loaded None.
    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .profile()
                .filter(Profile::fields().bio().eq("private bio")),
        )
        .get(&mut db)
        .await?;
    assert!(loaded.profile.get().is_none());

    // Filter matches → loaded Some.
    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .profile()
                .filter(Profile::fields().bio().eq("public bio")),
        )
        .get(&mut db)
        .await?;
    assert_eq!("public bio", loaded.profile.get().as_ref().unwrap().bio);

    Ok(())
}
