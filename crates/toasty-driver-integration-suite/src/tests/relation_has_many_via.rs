//! Multi-step (`via`) has_many relations: a `has_many` reached by following a
//! path of existing relations rather than a single foreign key.
//!
//! The shape under test is `User` → `Comment` → `Article`: a user has many
//! comments, each comment belongs to an article, so a user has many
//! `commented_articles` via `comments.article`.

use crate::prelude::*;

/// Querying a `via` relation returns the distinct targets reachable through
/// the path — a target is listed once however many intermediates reach it.
#[driver_test(id(ID), scenario(crate::scenarios::user_comment_article))]
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
#[driver_test(id(ID), scenario(crate::scenarios::user_comment_article))]
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
    requires(scan),
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
