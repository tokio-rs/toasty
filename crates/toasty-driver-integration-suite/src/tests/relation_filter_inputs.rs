use crate::prelude::*;

#[driver_test(requires(scan), scenario(crate::scenarios::user_profile_settings))]
pub async fn combined_membership(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
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
        let found = User::filter(filter).exec(&mut db).await?;
        assert_struct!(found, [{ name: == expected }]);
    }

    Ok(())
}

#[driver_test(requires(scan))]
pub async fn partitioned_membership(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = id)]
    struct Source {
        #[auto]
        id: uuid::Uuid,
        group: String,
        document_id: Option<uuid::Uuid>,
        #[belongs_to(key = document_id, references = id)]
        document: toasty::Deferred<Option<Document>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }

    let mut db = t.setup_db(models!(Source, Document)).await;
    let missing = toasty::create!(Source { group: "selected" })
        .exec(&mut db)
        .await?;
    toasty::create!(Source { group: "other" })
        .exec(&mut db)
        .await?;

    let found = Source::filter_by_group("selected")
        .filter(Source::fields().document().in_query(Document::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ id: == missing.id }]);

    let present = toasty::create!(Source {
        group: "selected",
        document: {}
    })
    .exec(&mut db)
    .await?;
    let found = Source::filter_by_group("selected")
        .filter(Source::fields().document().in_query(Document::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ id: == missing.id }]);
    let found = Source::filter_by_group("selected")
        .filter(Source::fields().document().in_query(Document::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ id: == present.id }]);
    Ok(())
}
