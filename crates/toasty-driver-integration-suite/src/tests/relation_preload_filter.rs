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

/// Filtering a `BelongsTo` include: the predicate is evaluated against the
/// parent row. When it excludes the parent the relation loads as `None`;
/// when it matches, as `Some`.
#[driver_test(id(ID), scenario(crate::scenarios::has_one_optional_belongs_to))]
pub async fn preload_belongs_to_with_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let _user = toasty::create!(User {
        name: "alice",
        profile: { bio: "public bio" }
    })
    .exec(&mut db)
    .await?;

    let mut profiles: Vec<Profile> = Profile::all().exec(&mut db).await?;
    let profile = profiles.remove(0);

    // Filter excludes the parent → loaded None.
    let loaded = Profile::filter_by_id(profile.id)
        .include(
            Profile::fields()
                .user()
                .filter(User::fields().name().eq("bob")),
        )
        .get(&mut db)
        .await?;
    assert!(loaded.user.get().is_none());

    // Filter matches → loaded Some.
    let loaded = Profile::filter_by_id(profile.id)
        .include(
            Profile::fields()
                .user()
                .filter(User::fields().name().eq("alice")),
        )
        .get(&mut db)
        .await?;
    assert_eq!("alice", loaded.user.get().as_ref().unwrap().name);

    Ok(())
}

/// A filter on a nested include applies to the innermost relation only. The
/// intermediate relation still loads in full; only its children are filtered.
#[driver_test]
pub async fn preload_nested_relation_with_filter(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
        title: String,
        #[has_many]
        comments: toasty::Deferred<Vec<Comment>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        post_id: uuid::Uuid,
        #[belongs_to(key = post_id, references = id)]
        post: toasty::Deferred<Post>,
        body: String,
    }

    let mut db = test.setup_db(models!(User, Post, Comment)).await;

    let user = toasty::create!(User { name: "alice" })
        .exec(&mut db)
        .await?;
    let post = toasty::create!(Post {
        title: "p1",
        user: &user
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Comment {
        body: "keep",
        post: &post
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Comment {
        body: "drop",
        post: &post
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

    // The post loads regardless of the filter; only its comments are filtered.
    let posts = loaded.posts.get();
    assert_eq!(1, posts.len());
    let bodies: Vec<&str> = posts[0]
        .comments
        .get()
        .iter()
        .map(|c| c.body.as_str())
        .collect();
    assert_eq!(bodies, vec!["keep"]);

    Ok(())
}

/// A bare `.include(todos())` (load all) combined with a filtered
/// `.include(todos().filter(...))` on the same relation loads the filtered
/// set: the predicate wins. An unfiltered include contributes no predicate,
/// so it does not broaden the result back to all.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_bare_and_filtered_uses_filter(test: &mut Test) -> Result<()> {
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

    let titles: Vec<&str> = alice_loaded
        .todos
        .get()
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, vec!["b"]);

    Ok(())
}

/// Filters at both levels of a nested include apply to their own relation:
/// the intermediate filter selects which posts load, and the innermost filter
/// selects which comments travel with each loaded post.
#[driver_test]
pub async fn preload_nested_relation_filters_at_both_levels(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
        title: String,
        #[has_many]
        comments: toasty::Deferred<Vec<Comment>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        post_id: uuid::Uuid,
        #[belongs_to(key = post_id, references = id)]
        post: toasty::Deferred<Post>,
        body: String,
    }

    let mut db = test.setup_db(models!(User, Post, Comment)).await;

    let user = toasty::create!(User { name: "alice" })
        .exec(&mut db)
        .await?;
    let keep_post = toasty::create!(Post {
        title: "keep",
        user: &user
    })
    .exec(&mut db)
    .await?;
    let drop_post = toasty::create!(Post {
        title: "drop",
        user: &user
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Comment::[
        { body: "keep", post: &keep_post },
        { body: "drop", post: &keep_post },
        { body: "keep", post: &drop_post },
    ])
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

    // The intermediate filter drops the `drop` post entirely; the innermost
    // filter drops the `drop` comment from the surviving post.
    let posts = loaded.posts.get();
    assert_eq!(1, posts.len());
    assert_eq!("keep", posts[0].title);
    let bodies: Vec<&str> = posts[0]
        .comments
        .get()
        .iter()
        .map(|c| c.body.as_str())
        .collect();
    assert_eq!(bodies, vec!["keep"]);

    Ok(())
}

/// An eager (non-deferred) relation is an implicit bare include, so a
/// filtered `.include(...)` of it restricts what loads — the implicit
/// include contributes no predicate and does not broaden the result.
#[driver_test]
pub async fn preload_eager_has_many_with_filter(test: &mut Test) -> Result<()> {
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

    // Eager baseline: no include, all todos load.
    let loaded = User::filter_by_id(alice.id).get(&mut db).await?;
    assert_eq!(3, loaded.todos.len());

    let loaded = User::filter_by_id(alice.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().eq("b")),
        )
        .get(&mut db)
        .await?;
    let titles: Vec<&str> = loaded.todos.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["b"]);

    Ok(())
}

/// Filtering a non-optional 1-1 relation is rejected upfront: there is no
/// `None` to load a non-matching row into, so whether the query would
/// succeed would depend on the data. The query fails with
/// `invalid_statement` regardless of whether the predicate matches.
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

    // Rejected even when the predicate would match every row.
    let err = User::filter_by_id(user.id)
        .include(
            User::fields()
                .profile()
                .filter(Profile::fields().bio().eq("real bio")),
        )
        .get(&mut db)
        .await
        .unwrap_err();
    assert!(
        err.is_invalid_statement(),
        "expected invalid_statement error, got: {err:?}"
    );

    Ok(())
}

/// Filtering the include of a required `BelongsTo` is rejected for the same
/// reason as a required `HasOne`: the loaded value cannot represent a
/// non-matching parent row.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_required_belongs_to_filter_rejected(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let _alice = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }]
    })
    .exec(&mut db)
    .await?;

    let mut todos: Vec<Todo> = Todo::all().exec(&mut db).await?;
    let todo = todos.remove(0);

    let err = Todo::filter_by_id(todo.id)
        .include(
            Todo::fields()
                .user()
                .filter(User::fields().name().eq("alice")),
        )
        .get(&mut db)
        .await
        .unwrap_err();
    assert!(
        err.is_invalid_statement(),
        "expected invalid_statement error, got: {err:?}"
    );

    Ok(())
}
