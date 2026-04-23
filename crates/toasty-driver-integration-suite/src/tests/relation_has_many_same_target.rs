//! Test has_many/belongs_to associations where multiple `belongs_to` fields on
//! the same child model target the same parent model, disambiguated by
//! `#[has_many(pair = <field>)]`.

use crate::prelude::*;
use hashbrown::HashSet;

#[driver_test(id(ID), scenario(crate::scenarios::has_many_same_target))]
pub async fn pair_hint_disambiguates_has_many(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = User::create().name("Alice").exec(&mut db).await?;
    let bob = User::create().name("Bob").exec(&mut db).await?;

    // Alice authors two articles, each reviewed by Bob.
    let a1 = Article::create()
        .title("one")
        .author(&alice)
        .reviewer(&bob)
        .exec(&mut db)
        .await?;
    let a2 = Article::create()
        .title("two")
        .author(&alice)
        .reviewer(&bob)
        .exec(&mut db)
        .await?;

    // Bob authors one article, reviewed by Alice.
    let a3 = Article::create()
        .title("three")
        .author(&bob)
        .reviewer(&alice)
        .exec(&mut db)
        .await?;

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

    let alice = User::create().name("Alice").exec(&mut db).await?;
    let bob = User::create().name("Bob").exec(&mut db).await?;

    // Create an article scoped to Alice's authored side — the other FK still
    // needs a concrete reviewer, and the side you create through should be
    // filled by the accessor.
    let article = alice
        .authored_articles()
        .create()
        .title("draft")
        .reviewer(&bob)
        .exec(&mut db)
        .await?;

    assert_eq!(article.author_id, alice.id);
    assert_eq!(article.reviewer_id, bob.id);

    // The reviewer side for Alice is still empty even though she has an
    // authored article.
    assert!(alice.reviewed_articles().exec(&mut db).await?.is_empty());

    Ok(())
}
