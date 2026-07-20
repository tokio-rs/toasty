use crate::prelude::*;

/// Filtering a `HasMany` include loads only matching children. Parents
/// with no matching children are returned with an empty preloaded `Vec`.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        {
            name: "alice",
            todos: [{ title: "a" }, { title: "b" }, { title: "c" }]
        },
        { name: "bob", todos: [{ title: "x" }] },
    ])
    .exec(&mut db)
    .await?;

    let users: Vec<User> = User::all()
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("b")),
        )
        .exec(&mut db)
        .await?;

    assert_struct!(users, #(
        { name: "alice", todos.get(): [{ title: "b" }] },
        { name: "bob", todos.get(): [] },
    ));

    Ok(())
}

/// An include filter is independent of a parent-side `.any(...)`. The
/// parent filter selects which users come back; the include filter
/// selects which todos travel with each user.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_filter_independent_of_parent_any(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        {
            name: "alice",
            todos: [{ title: "keep" }, { title: "drop" }]
        },
        { name: "bob", todos: [{ title: "drop" }] },
    ])
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

    assert_struct!(users, [{
        name: "alice",
        todos.get(): [{ title: "keep" }],
    }]);

    Ok(())
}

/// Repeated includes of the same relation OR their filters.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_repeated_filters_ored(test: &mut Test) -> Result<()> {
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
                .filter(Todo::fields().title().ne("aaa")),
        )
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("aab")),
        )
        .get(&mut db)
        .await?;

    assert_struct!(alice_loaded.todos.get(), #(
        { title: "aab" },
        { title: "bbb" },
    ));

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

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .profile()
                .filter(Profile::fields().bio().eq("private bio")),
        )
        .get(&mut db)
        .await?;
    assert_struct!(loaded, { profile.get(): None });

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .profile()
                .filter(Profile::fields().bio().eq("public bio")),
        )
        .get(&mut db)
        .await?;
    assert_struct!(loaded, { profile.get(): Some({ bio: "public bio" }) });

    Ok(())
}

/// Filtering a `BelongsTo` include: the predicate is evaluated against the
/// parent row. When it excludes the parent the relation loads as `None`;
/// when it matches, as `Some`.
#[driver_test(id(ID), scenario(crate::scenarios::has_one_optional_belongs_to))]
pub async fn preload_belongs_to_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User {
        name: "alice",
        profile: { bio: "public bio" }
    })
    .exec(&mut db)
    .await?;

    let profile = Profile::all().get(&mut db).await?;

    let loaded = Profile::filter_by_id(profile.id)
        .include(
            Profile::fields()
                .user()
                .filter(User::fields().name().eq("bob")),
        )
        .get(&mut db)
        .await?;
    assert_struct!(loaded, { user.get(): None });

    let loaded = Profile::filter_by_id(profile.id)
        .include(
            Profile::fields()
                .user()
                .filter(User::fields().name().eq("alice")),
        )
        .get(&mut db)
        .await?;
    assert_struct!(loaded, { user.get(): Some({ name: "alice" }) });

    Ok(())
}

/// A filter on a nested include applies to the innermost relation only. The
/// intermediate relation still loads in full; only its children are filtered.
#[driver_test(id(ID), scenario(crate::scenarios::user_post_comment))]
pub async fn preload_nested_relation_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        posts: [{
            title: "p1",
            comments: [{ body: "keep" }, { body: "drop" }]
        }]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .posts()
                .comments()
                .filter(Comment::fields().body().eq("keep")),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.posts.get(), [{
        comments.get(): [{ body: "keep" }],
    }]);

    Ok(())
}

/// A bare include dominates a filtered include of the same relation.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_bare_and_filtered_loads_all(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "b" }, { title: "c" }]
    })
    .exec(&mut db)
    .await?;

    let alice_loaded = User::filter_by_id(alice.id)
        .include(User::fields().todos())
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("b")),
        )
        .get(&mut db)
        .await?;

    assert_struct!(alice_loaded.todos.get(), #(
        { title: "a" },
        { title: "b" },
        { title: "c" },
    ));

    Ok(())
}

/// Nested filters apply at their own relation level.
#[driver_test(id(ID), scenario(crate::scenarios::user_post_comment))]
pub async fn preload_nested_relation_filters_at_both_levels(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        posts: [
            {
                title: "keep",
                comments: [{ body: "keep" }, { body: "drop" }]
            },
            {
                title: "drop",
                comments: [{ body: "keep" }]
            },
        ]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .posts()
                .filter(Post::fields().title().eq("keep")),
        )
        .include(
            User::fields()
                .posts()
                .comments()
                .filter(Comment::fields().body().eq("keep")),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.posts.get(), [{
        title: "keep",
        comments.get(): [{ body: "keep" }],
    }]);

    Ok(())
}

/// An eager relation's implicit bare include dominates a filtered include.
#[driver_test]
pub async fn preload_eager_has_many_filter_still_loads_all(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        todos: Vec<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let alice = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "b" }, { title: "c" }]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(alice.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("b")),
        )
        .get(&mut db)
        .await?;
    assert_struct!(loaded.todos, #(
        { title: "a" },
        { title: "b" },
        { title: "c" },
    ));

    Ok(())
}

/// Filtering a required one-to-one relation is rejected.
#[driver_test]
pub async fn preload_required_has_one_filter_rejected(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_one]
        profile: Profile,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
        bio: String,
    }

    let mut db = test.setup_db(models!(User, Profile)).await;

    let user = toasty::create!(User {
        name: "alice",
        profile: { bio: "real bio" }
    })
    .exec(&mut db)
    .await?;

    let err = assert_err!(
        User::filter_by_id(user.id)
            .include(
                User::fields()
                    .profile()
                    .filter(Profile::fields().bio().eq("real bio")),
            )
            .get(&mut db)
            .await
    );
    assert!(
        err.is_invalid_statement(),
        "expected invalid_statement error, got: {err:?}"
    );

    Ok(())
}

/// Filtering a required `BelongsTo` relation is rejected.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_required_belongs_to_filter_rejected(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }]
    })
    .exec(&mut db)
    .await?;

    let todo = Todo::all().get(&mut db).await?;

    let err = assert_err!(
        Todo::filter_by_id(todo.id)
            .include(
                Todo::fields()
                    .user()
                    .filter(User::fields().name().eq("alice")),
            )
            .get(&mut db)
            .await
    );
    assert!(
        err.is_invalid_statement(),
        "expected invalid_statement error, got: {err:?}"
    );

    Ok(())
}
