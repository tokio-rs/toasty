//! Multi-step (`via`) has_many relations: a `has_many` reached by following a
//! path of existing relations rather than a single foreign key.
//!
//! The shape under test is `User` → `Comment` → `Article`: a user has many
//! comments, each comment belongs to an article, so a user has many
//! `commented_articles` via `comments.article`.

use crate::prelude::*;

/// Querying a `via` relation returns the distinct targets reachable through
/// the path — a target is listed once however many intermediates reach it.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_comment_article)
)]
pub async fn query_returns_distinct_targets(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob) = (&users[0], &users[1]);

    let articles = toasty::create!(Article::[
        { title: "Rust" },
        { title: "Toasty" },
        { title: "SQL" },
    ])
    .exec(&mut db)
    .await?;
    let (rust, toasty_article, sql) = (&articles[0], &articles[1], &articles[2]);

    // Alice comments on Rust twice and Toasty once; Bob comments on SQL.
    toasty::create!(Comment::[
        { body: "a1", user: alice, article: rust },
        { body: "a2", user: alice, article: rust },
        { body: "a3", user: alice, article: toasty_article },
        { body: "b1", user: bob, article: sql },
    ])
    .exec(&mut db)
    .await?;

    // Alice has commented on Rust and Toasty. Rust appears once even though
    // she commented on it twice — `via` yields distinct targets.
    let commented = alice.commented_articles().exec(&mut db).await?;
    assert_eq_unordered!(commented.iter().map(|a| &a.title[..]), ["Rust", "Toasty"]);

    // Bob has commented only on SQL.
    let commented = bob.commented_articles().exec(&mut db).await?;
    assert_eq_unordered!(commented.iter().map(|a| &a.title[..]), ["SQL"]);

    Ok(())
}

/// A user with no comments reaches no articles — an empty result, no error.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_comment_article)
)]
pub async fn query_with_no_intermediates_is_empty(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(Article { title: "Rust" })
        .exec(&mut db)
        .await?;

    let commented = user.commented_articles().exec(&mut db).await?;
    assert!(commented.is_empty());

    Ok(())
}

/// A `via` relation query can be further filtered, like any other relation
/// query.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_comment_article)
)]
pub async fn via_relation_query_can_be_filtered(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let articles = toasty::create!(Article::[
        { title: "Rust" },
        { title: "Toasty" },
        { title: "SQL" },
    ])
    .exec(&mut db)
    .await?;
    let (rust, toasty_article, sql) = (&articles[0], &articles[1], &articles[2]);

    toasty::create!(Comment::[
        { body: "a1", user: &alice, article: rust },
        { body: "a2", user: &alice, article: toasty_article },
        { body: "a3", user: &alice, article: sql },
    ])
    .exec(&mut db)
    .await?;

    let filtered: Vec<_> = alice
        .commented_articles()
        .filter(Article::fields().title().eq("Toasty"))
        .exec(&mut db)
        .await?;
    assert_eq_unordered!(filtered.iter().map(|a| &a.title[..]), ["Toasty"]);

    Ok(())
}

// ===== `.include()` of multi-step `via` relations =====
//
// The two scenarios below cover include over via paths of different lengths:
//
//   - `user_comment_article`        — 2 steps (HasMany → BelongsTo)
//   - `user_org_project_todo`       — 3 steps (HasMany → HasMany → HasMany),
//                                     plus a via-of-via whose path step is
//                                     itself a via.
//
// The engine should fetch parents once, then issue a single child query that
// `INNER JOIN`s each intermediate model and groups results by the parent FK.

/// `.include()` over a 2-step `via`: each parent gets its own filtered set
/// of distinct targets reached through the path. Tests the HasMany →
/// BelongsTo shape (User → Comment → Article).
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_comment_article)
)]
pub async fn include_via_two_step(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
        { name: "Charlie" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob, _charlie) = (&users[0], &users[1], &users[2]);

    let articles = toasty::create!(Article::[
        { title: "Rust" },
        { title: "Toasty" },
        { title: "SQL" },
    ])
    .exec(&mut db)
    .await?;
    let (rust, toasty_article, sql) = (&articles[0], &articles[1], &articles[2]);

    // Alice → Rust (twice), Toasty.  Bob → SQL.  Charlie → nothing.
    toasty::create!(Comment::[
        { body: "a1", user: alice, article: rust },
        { body: "a2", user: alice, article: rust },
        { body: "a3", user: alice, article: toasty_article },
        { body: "b1", user: bob, article: sql },
    ])
    .exec(&mut db)
    .await?;

    let loaded: Vec<User> = User::all()
        .include(User::fields().commented_articles())
        .exec(&mut db)
        .await?;
    assert_eq!(3, loaded.len());

    for user in &loaded {
        let titles: Vec<&str> = user
            .commented_articles
            .get()
            .iter()
            .map(|a| &a.title[..])
            .collect();
        match &user.name[..] {
            // Alice commented on Rust twice but `via` yields distinct
            // targets, so Rust appears once.
            "Alice" => {
                assert_eq_unordered!(titles, ["Rust", "Toasty"]);
            }
            "Bob" => {
                assert_eq_unordered!(titles, ["SQL"]);
            }
            "Charlie" => assert!(titles.is_empty(), "Charlie has no comments; got {titles:?}"),
            other => panic!("unexpected user {other}"),
        }
    }

    Ok(())
}

/// `.include()` over a 3-step `via`: User → Organization → Project → Todo,
/// all `HasMany` steps. Verifies that the child query joins every
/// intermediate and groups todos by the root user.
///
/// The data shape (Alice has two orgs, one with two projects; Bob one org with
/// one project; each project has a couple of todos) is shared with
/// [`include_via_nested_via`] so the two can be compared directly. It can't be
/// hoisted into a helper: the `id(ID)` expansion generates per-ID-type model
/// structs, so `User`/`Todo`/etc. only exist inside a scenario-scoped test fn.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_org_project_todo)
)]
pub async fn include_via_three_step(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob) = (&users[0], &users[1]);

    let alice_org_a = toasty::create!(Organization {
        name: "A-Co",
        user: alice
    })
    .exec(&mut db)
    .await?;
    let alice_org_b = toasty::create!(Organization {
        name: "B-Co",
        user: alice
    })
    .exec(&mut db)
    .await?;
    let bob_org = toasty::create!(Organization {
        name: "Bob-Inc",
        user: bob
    })
    .exec(&mut db)
    .await?;

    let alice_proj_1 = toasty::create!(Project {
        name: "p1",
        organization: &alice_org_a
    })
    .exec(&mut db)
    .await?;
    let alice_proj_2 = toasty::create!(Project {
        name: "p2",
        organization: &alice_org_a
    })
    .exec(&mut db)
    .await?;
    let alice_proj_3 = toasty::create!(Project {
        name: "p3",
        organization: &alice_org_b
    })
    .exec(&mut db)
    .await?;
    let bob_proj = toasty::create!(Project {
        name: "bp",
        organization: &bob_org
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Todo::[
        { title: "a-1", project: &alice_proj_1 },
        { title: "a-2", project: &alice_proj_1 },
        { title: "a-3", project: &alice_proj_2 },
        { title: "a-4", project: &alice_proj_3 },
        { title: "b-1", project: &bob_proj },
        { title: "b-2", project: &bob_proj },
    ])
    .exec(&mut db)
    .await?;

    let loaded: Vec<User> = User::all()
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;
    assert_eq!(2, loaded.len());

    for user in &loaded {
        let titles: Vec<&str> = user.todos.get().iter().map(|t| &t.title[..]).collect();
        match &user.name[..] {
            "Alice" => {
                assert_eq_unordered!(titles, ["a-1", "a-2", "a-3", "a-4"]);
            }
            "Bob" => {
                assert_eq_unordered!(titles, ["b-1", "b-2"]);
            }
            other => panic!("unexpected user {other}"),
        }
    }

    Ok(())
}

/// `.include()` over a via-of-via: `User::nested_todos` reaches todos through
/// `organizations.todos`, where `Organization::todos` is itself a via. The
/// outer path's second step expands into a nested via during lowering, so this
/// exercises recursive via flattening. The result must match the flat 3-step
/// `User::todos` include in [`include_via_three_step`] exactly — same data
/// shape, same expected grouping.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_org_project_todo)
)]
pub async fn include_via_nested_via(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob) = (&users[0], &users[1]);

    let alice_org_a = toasty::create!(Organization {
        name: "A-Co",
        user: alice
    })
    .exec(&mut db)
    .await?;
    let alice_org_b = toasty::create!(Organization {
        name: "B-Co",
        user: alice
    })
    .exec(&mut db)
    .await?;
    let bob_org = toasty::create!(Organization {
        name: "Bob-Inc",
        user: bob
    })
    .exec(&mut db)
    .await?;

    let alice_proj_1 = toasty::create!(Project {
        name: "p1",
        organization: &alice_org_a
    })
    .exec(&mut db)
    .await?;
    let alice_proj_2 = toasty::create!(Project {
        name: "p2",
        organization: &alice_org_a
    })
    .exec(&mut db)
    .await?;
    let alice_proj_3 = toasty::create!(Project {
        name: "p3",
        organization: &alice_org_b
    })
    .exec(&mut db)
    .await?;
    let bob_proj = toasty::create!(Project {
        name: "bp",
        organization: &bob_org
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Todo::[
        { title: "a-1", project: &alice_proj_1 },
        { title: "a-2", project: &alice_proj_1 },
        { title: "a-3", project: &alice_proj_2 },
        { title: "a-4", project: &alice_proj_3 },
        { title: "b-1", project: &bob_proj },
        { title: "b-2", project: &bob_proj },
    ])
    .exec(&mut db)
    .await?;

    let loaded: Vec<User> = User::all()
        .include(User::fields().nested_todos())
        .exec(&mut db)
        .await?;
    assert_eq!(2, loaded.len());

    for user in &loaded {
        let titles: Vec<&str> = user
            .nested_todos
            .get()
            .iter()
            .map(|t| &t.title[..])
            .collect();
        match &user.name[..] {
            "Alice" => {
                assert_eq_unordered!(titles, ["a-1", "a-2", "a-3", "a-4"]);
            }
            "Bob" => {
                assert_eq_unordered!(titles, ["b-1", "b-2"]);
            }
            other => panic!("unexpected user {other}"),
        }
    }

    Ok(())
}

/// A user with no intermediates yields an empty included set — the
/// `INNER JOIN` excludes them but the parent row is still returned.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_org_project_todo)
)]
pub async fn include_via_three_step_no_intermediates(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let loaded = User::filter_by_id(alice.id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;
    assert!(loaded.todos.get().is_empty());

    Ok(())
}
