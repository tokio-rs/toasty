use crate::prelude::*;

#[driver_test(requires(scan), scenario(crate::scenarios::user_profile_settings))]
pub async fn key_lookup_with_membership(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let users = toasty::create!(User::[
        { name: "neither" },
        { name: "profile", profile: { bio: "profile" } },
        { name: "settings", settings: { theme: "dark" } },
        { name: "both", profile: { bio: "both" }, settings: { theme: "light" } },
    ])
    .exec(&mut db)
    .await?;

    for (filter, expected) in [
        (
            User::fields()
                .profile()
                .in_query(Profile::all())
                .not()
                .and(User::fields().settings().in_query(Settings::all()).not()),
            "neither",
        ),
        (
            User::fields()
                .profile()
                .in_query(Profile::all())
                .and(User::fields().settings().in_query(Settings::all()).not()),
            "profile",
        ),
        (
            User::fields()
                .settings()
                .in_query(Settings::all())
                .and(User::fields().profile().in_query(Profile::all()).not()),
            "settings",
        ),
    ] {
        // A key lookup applies the subquery predicates after loading the row.
        for user in &users {
            let found = User::filter_by_id(user.id)
                .filter(filter.clone())
                .exec(&mut db)
                .await?;
            assert_eq!(found.len(), usize::from(user.name == expected));
        }
    }

    Ok(())
}

#[driver_test(requires(scan), scenario(crate::scenarios::user_profile_settings))]
pub async fn key_update_with_membership(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let users = toasty::create!(User::[
        { name: "absent" },
        { name: "present", profile: { bio: "profile" } },
    ])
    .exec(&mut db)
    .await?;

    for user in &users {
        User::filter_by_id(user.id)
            .filter(User::fields().profile().in_query(Profile::all()))
            .update()
            .name("updated")
            .exec(&mut db)
            .await?;

        let found = User::filter_by_id(user.id).exec(&mut db).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].name,
            if user.name == "present" {
                "updated"
            } else {
                "absent"
            },
        );
    }
    Ok(())
}

#[driver_test(requires(scan), scenario(crate::scenarios::two_models))]
pub async fn key_delete_with_subquery(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let users = toasty::create!(User::[
        { name: "excluded" },
        { name: "selected" },
    ])
    .exec(&mut db)
    .await?;
    toasty::create!(Post {
        id: users[1].id,
        title: "candidate"
    })
    .exec(&mut db)
    .await?;

    for user in &users {
        let candidates = Post::all().select(Post::fields().id());
        User::filter_by_id(user.id)
            .filter(User::fields().id().in_query(candidates))
            .delete()
            .exec(&mut db)
            .await?;

        let found = User::filter_by_id(user.id).exec(&mut db).await?;
        assert_eq!(found.len(), usize::from(user.name == "excluded"));
    }
    Ok(())
}
