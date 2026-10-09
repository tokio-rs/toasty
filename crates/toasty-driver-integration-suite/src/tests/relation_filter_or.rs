use crate::prelude::*;

#[driver_test(requires(scan), scenario(crate::scenarios::user_profile_settings))]
pub async fn combined_membership_query(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "neither" },
        { name: "profile", profile: { bio: "profile" } },
        { name: "settings", settings: { theme: "dark" } },
        { name: "both", profile: { bio: "both" }, settings: { theme: "light" } },
    ])
    .exec(&mut db)
    .await?;

    let found = User::filter(
        User::fields()
            .profile()
            .in_query(Profile::all())
            .or(User::fields().settings().in_query(Settings::all())),
    )
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(
        found.iter().map(|user| user.name.as_str()),
        ["profile", "settings", "both"]
    );
    Ok(())
}

#[driver_test(
    requires(and(not(sql), not(index_or_predicate))),
    scenario(crate::scenarios::user_profile_settings)
)]
pub async fn combined_membership_mutation_requires_index(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User { name: "unchanged", profile: { bio: "profile" } })
        .exec(&mut db)
        .await?;

    let filter = User::fields()
        .profile()
        .in_query(Profile::all())
        .or(User::fields().settings().in_query(Settings::all()));
    let err = User::filter(filter.clone())
        .update()
        .name("changed")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(err.is_unsupported_feature(), "{err:?}");
    let err = User::filter(filter)
        .delete()
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(err.is_unsupported_feature(), "{err:?}");
    let user = User::get_by_id(&mut db, user.id).await?;
    assert_eq!(user.name, "unchanged");
    Ok(())
}
