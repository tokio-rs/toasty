//! Tests for filtering models by conditions on associated (HasOne, BelongsTo) fields.

use crate::prelude::*;

/// Filter a parent by a HasOne field, with eq and a non-commutative op.
#[driver_test(id(ID), requires(sql))]
pub async fn filter_by_has_one_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        score: i64,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let mut db = t.setup_db(models!(User, Profile)).await;

    toasty::create!(User::[
        { name: "alice", profile: { score: 80 } },
        { name: "bob",   profile: { score: 30 } },
    ])
    .exec(&mut db)
    .await?;

    let users: Vec<User> = User::filter(User::fields().profile().score().eq(80))
        .exec(&mut db)
        .await?;
    assert_eq!(
        users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
        ["alice"]
    );

    let users: Vec<User> = User::filter(User::fields().profile().score().gt(50))
        .exec(&mut db)
        .await?;
    assert_eq!(
        users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
        ["alice"]
    );

    Ok(())
}

/// Filter a child by a BelongsTo field.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::has_one_optional_belongs_to)
)]
pub async fn filter_by_belongs_to_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        { name: "alice", profile: { bio: "alice's bio" } },
        { name: "bob",   profile: { bio: "bob's bio" } },
    ])
    .exec(&mut db)
    .await?;

    let profiles: Vec<Profile> = Profile::filter(Profile::fields().user().name().eq("alice"))
        .exec(&mut db)
        .await?;
    assert_eq!(
        profiles.iter().map(|p| p.bio.as_str()).collect::<Vec<_>>(),
        ["alice's bio"]
    );

    Ok(())
}

/// Filter through three chained HasOne associations: `A.b.c.name == ...`.
#[driver_test(id(ID), requires(sql))]
pub async fn filter_by_nested_has_one_chain(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct A {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[has_one]
        b: toasty::HasOne<Option<B>>,
    }

    #[derive(Debug, toasty::Model)]
    struct B {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        a_id: Option<ID>,

        #[belongs_to(key = a_id, references = id)]
        a: toasty::BelongsTo<Option<A>>,

        #[has_one]
        c: toasty::HasOne<Option<C>>,
    }

    #[derive(Debug, toasty::Model)]
    struct C {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[unique]
        b_id: Option<ID>,

        #[belongs_to(key = b_id, references = id)]
        b: toasty::BelongsTo<Option<B>>,
    }

    let mut db = t.setup_db(models!(A, B, C)).await;

    toasty::create!(A::[
        { label: "match",    b: { c: { name: "target" } } },
        { label: "no-match", b: { c: { name: "other" } } },
    ])
    .exec(&mut db)
    .await?;

    let results: Vec<A> = A::filter(A::fields().b().c().name().eq("target"))
        .exec(&mut db)
        .await?;
    assert_eq!(
        results.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(),
        ["match"]
    );

    Ok(())
}
