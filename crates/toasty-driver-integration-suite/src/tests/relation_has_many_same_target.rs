//! Test has_many/belongs_to associations where multiple `belongs_to` fields on
//! the same child model target the same parent model, disambiguated by
//! `#[has_many(pair = <field>)]`.

use crate::prelude::*;
use hashbrown::HashSet;

#[driver_test(id(ID), scenario(crate::scenarios::has_many_same_target))]
pub async fn pair_hint_disambiguates_has_many(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob) = (&users[0], &users[1]);

    // Alice authors two articles (both reviewed by Bob); Bob authors one
    // (reviewed by Alice).
    let articles = toasty::create!(Article::[
        { title: "one",   author: alice, reviewer: bob },
        { title: "two",   author: alice, reviewer: bob },
        { title: "three", author: bob,   reviewer: alice },
    ])
    .exec(&mut db)
    .await?;
    let (a1, a2, a3) = (&articles[0], &articles[1], &articles[2]);

    // Each has_many side picks up only the articles that match its paired
    // belongs_to — not every article referencing the user.
    let alice_authored: HashSet<_> = alice
        .authored_articles()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(alice_authored, HashSet::from_iter([a1.id, a2.id]));

    let alice_reviewed: HashSet<_> = alice
        .reviewed_articles()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(alice_reviewed, HashSet::from_iter([a3.id]));

    // Navigating back from an article to each parent resolves the correct user.
    let a1_author = a1.author().exec(&mut db).await?;
    let a1_reviewer = a1.reviewer().exec(&mut db).await?;
    assert_eq!(a1_author.id, alice.id);
    assert_eq!(a1_reviewer.id, bob.id);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_same_target))]
pub async fn pair_hint_create_via_has_many_accessor(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob) = (&users[0], &users[1]);

    // Create an article scoped to Alice's authored side — the scoped create
    // should fill in the `author` FK, and the other side (`reviewer`) still
    // needs to be specified.
    let article = toasty::create!(in alice.authored_articles() {
        title: "draft",
        reviewer: bob,
    })
    .exec(&mut db)
    .await?;

    assert_eq!(article.author_id, alice.id);
    assert_eq!(article.reviewer_id, bob.id);

    // The reviewer side for Alice is still empty even though she has an
    // authored article.
    assert!(alice.reviewed_articles().exec(&mut db).await?.is_empty());

    Ok(())
}
