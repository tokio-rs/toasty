//! Many-to-many relations use a join model with a `has_many` relation on each
//! endpoint and a `has_many(via = ...)` relation for direct traversal.

use crate::prelude::*;

/// Both endpoints can traverse the same join rows. The derived query retains
/// normal query operations such as ordering and returns an empty list when no
/// join rows exist.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_group_membership)
)]
pub async fn query_from_both_sides(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let fixture = seed(&mut db).await?;

    let groups = fixture
        .alice
        .groups()
        .order_by(Group::fields().name().asc())
        .exec(&mut db)
        .await?;
    assert_eq!(
        groups
            .iter()
            .map(|group| &group.name[..])
            .collect::<Vec<_>>(),
        ["Databases", "Rust"]
    );

    let users = fixture
        .rust
        .users()
        .order_by(User::fields().name().asc())
        .exec(&mut db)
        .await?;
    assert_eq!(
        users.iter().map(|user| &user.name[..]).collect::<Vec<_>>(),
        ["Alice", "Bob"]
    );

    assert!(fixture.carol.groups().exec(&mut db).await?.is_empty());
    assert!(fixture.empty.users().exec(&mut db).await?.is_empty());

    Ok(())
}

/// Parent queries can use `.any()` on the join relation. The predicate can
/// traverse to the opposite endpoint and can also inspect fields stored on the
/// join model.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_group_membership)
)]
pub async fn filter_through_join_model(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    seed(&mut db).await?;

    let users: Vec<User> = User::filter(
        User::fields()
            .memberships()
            .any(Membership::fields().group().name().eq("Rust")),
    )
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(users.iter().map(|user| &user.name[..]), ["Alice", "Bob"]);

    let groups: Vec<Group> = Group::filter(
        Group::fields()
            .memberships()
            .any(Membership::fields().user().name().eq("Alice")),
    )
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(
        groups.iter().map(|group| &group.name[..]),
        ["Rust", "Databases"]
    );

    let owners: Vec<User> = User::filter(
        User::fields()
            .memberships()
            .any(Membership::fields().role().eq("owner")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].name, "Alice");

    Ok(())
}

/// Parent queries can apply `.any()` directly to either derived relation,
/// without spelling the join model in the predicate.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_group_membership)
)]
pub async fn filter_through_via_from_both_sides(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    seed(&mut db).await?;

    let users: Vec<User> = User::filter(
        User::fields()
            .groups()
            .any(Group::fields().name().eq("Rust")),
    )
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(users.iter().map(|user| &user.name[..]), ["Alice", "Bob"]);

    let groups: Vec<Group> = Group::filter(
        Group::fields()
            .users()
            .any(User::fields().name().eq("Alice")),
    )
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(
        groups.iter().map(|group| &group.name[..]),
        ["Rust", "Databases"]
    );

    Ok(())
}

/// Preloading works in both directions and keeps results grouped under the
/// correct endpoint, including endpoints with no join rows.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_group_membership)
)]
pub async fn include_from_both_sides(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    seed(&mut db).await?;

    let users: Vec<User> = User::all()
        .include(User::fields().groups())
        .exec(&mut db)
        .await?;
    for user in &users {
        let groups = user.groups.get().iter().map(|group| &group.name[..]);
        match &user.name[..] {
            "Alice" => {
                assert_eq_unordered!(groups, ["Rust", "Databases"]);
            }
            "Bob" => {
                assert_eq_unordered!(groups, ["Rust"]);
            }
            "Carol" => {
                assert_eq!(groups.count(), 0);
            }
            name => panic!("unexpected user {name}"),
        }
    }

    let groups: Vec<Group> = Group::all()
        .include(Group::fields().users())
        .exec(&mut db)
        .await?;
    for group in &groups {
        let users = group.users.get().iter().map(|user| &user.name[..]);
        match &group.name[..] {
            "Rust" => {
                assert_eq_unordered!(users, ["Alice", "Bob"]);
            }
            "Databases" => {
                assert_eq_unordered!(users, ["Alice"]);
            }
            "Empty" => {
                assert_eq!(users.count(), 0);
            }
            name => panic!("unexpected group {name}"),
        }
    }

    Ok(())
}

/// Links are created, updated, and removed by mutating the join model. This
/// preserves relation data such as `role` and leaves both endpoint records
/// intact when the link is removed.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_group_membership)
)]
pub async fn mutate_link_through_join_model(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let group = toasty::create!(Group { name: "Rust" })
        .exec(&mut db)
        .await?;

    let mut membership = toasty::create!(Membership {
        user: &user,
        group: &group,
        role: "member"
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.groups().get(&mut db).await?.id, group.id);
    assert_eq!(group.users().get(&mut db).await?.id, user.id);

    membership.update().role("owner").exec(&mut db).await?;
    assert_eq!(membership.role, "owner");

    membership.delete().exec(&mut db).await?;
    assert!(user.groups().exec(&mut db).await?.is_empty());
    assert!(group.users().exec(&mut db).await?.is_empty());

    assert_eq!(User::get_by_id(&mut db, user.id).await?.name, "Alice");
    assert_eq!(Group::get_by_id(&mut db, group.id).await?.name, "Rust");

    Ok(())
}

/// A composite primary key on the join model prevents duplicate links while
/// allowing each endpoint to participate in other links.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_group_membership)
)]
pub async fn composite_join_key_prevents_duplicate_links(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let users = toasty::create!(User::[{ name: "Alice" }, { name: "Bob" }])
        .exec(&mut db)
        .await?;
    let groups = toasty::create!(Group::[{ name: "Rust" }, { name: "Databases" }])
        .exec(&mut db)
        .await?;

    toasty::create!(Membership {
        user: &users[0],
        group: &groups[0],
        role: "member"
    })
    .exec(&mut db)
    .await?;

    assert_err!(
        toasty::create!(Membership {
            user: &users[0],
            group: &groups[0],
            role: "owner"
        })
        .exec(&mut db)
        .await
    );

    toasty::create!(Membership::[
        { user: &users[0], group: &groups[1], role: "member" },
        { user: &users[1], group: &groups[0], role: "member" },
    ])
    .exec(&mut db)
    .await?;

    Ok(())
}

/// A self-referential many-to-many relation uses pair hints to distinguish
/// the two foreign keys on the join model. Each direction then exposes its
/// own `has_many(via = ...)` traversal.
#[driver_test(id(ID), requires(sql))]
pub async fn self_referential_followers_and_following(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many(pair = follower)]
        outgoing_follows: toasty::Deferred<Vec<Follow>>,

        #[has_many(pair = followed)]
        incoming_follows: toasty::Deferred<Vec<Follow>>,

        #[has_many(via = outgoing_follows.followed)]
        following: toasty::Deferred<Vec<User>>,

        #[has_many(via = incoming_follows.follower)]
        followers: toasty::Deferred<Vec<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(follower_id, followed_id)]
    struct Follow {
        #[index]
        follower_id: ID,

        #[belongs_to(key = follower_id, references = id)]
        follower: toasty::Deferred<User>,

        #[index]
        followed_id: ID,

        #[belongs_to(key = followed_id, references = id)]
        followed: toasty::Deferred<User>,
    }

    let mut db = test.setup_db(models!(User, Follow)).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
        { name: "Carol" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob, carol) = (&users[0], &users[1], &users[2]);

    toasty::create!(Follow::[
        { follower: alice, followed: bob   },
        { follower: alice, followed: carol },
        { follower: carol, followed: bob   },
    ])
    .exec(&mut db)
    .await?;

    let following = alice.following().exec(&mut db).await?;
    assert_eq_unordered!(
        following.iter().map(|user| &user.name[..]),
        ["Bob", "Carol"]
    );

    let followers = bob.followers().exec(&mut db).await?;
    assert_eq_unordered!(
        followers.iter().map(|user| &user.name[..]),
        ["Alice", "Carol"]
    );

    assert!(bob.following().exec(&mut db).await?.is_empty());

    Ok(())
}
