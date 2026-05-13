//! `.select(...)` projection through a `HasOne` relation field.
//!
//! Per PR #827, projecting a `BelongsTo` works because the macro emits
//! `IntoExpr<TargetModel>` for the relation field-struct and the lowering
//! walk routes the reference through `build_relation_subquery`.  `HasOne`
//! uses the same field-struct type (`<Target as Relation>::OneField`) and
//! `build_relation_subquery` already has a `HasOne` branch (used by
//! `.include`), so the case works end-to-end with no further production
//! code change.

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn select_has_one_basic(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let mut db = t.setup_db(models!(User, Profile)).await;

    toasty::create!(User {
        name: "Alice",
        profile: Profile::create().bio("apple a day"),
    })
    .exec(&mut db)
    .await?;

    let profiles: Vec<Profile> = User::all()
        .select(User::fields().profile())
        .exec(&mut db)
        .await?;

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].bio, "apple a day");

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn select_has_one_with_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let mut db = t.setup_db(models!(User, Profile)).await;

    toasty::create!(User {
        name: "Alice",
        profile: Profile::create().bio("alpha bio"),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Bob",
        profile: Profile::create().bio("beta bio"),
    })
    .exec(&mut db)
    .await?;

    let profiles: Vec<Profile> = User::filter(User::fields().name().eq("Bob"))
        .select(User::fields().profile())
        .exec(&mut db)
        .await?;

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].bio, "beta bio");

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn select_has_one_first(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let mut db = t.setup_db(models!(User, Profile)).await;

    toasty::create!(User {
        name: "Alice",
        profile: Profile::create().bio("apple a day"),
    })
    .exec(&mut db)
    .await?;

    let profile: Option<Profile> = User::filter(User::fields().name().eq("Alice"))
        .select(User::fields().profile())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(profile.map(|p| p.bio).as_deref(), Some("apple a day"));

    Ok(())
}
