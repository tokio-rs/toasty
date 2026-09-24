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
