use crate::prelude::*;

#[driver_test(
    requires(scan),
    scenario(crate::scenarios::has_one_optional_belongs_to::id_uuid)
)]
pub async fn has_one_membership_with_null_foreign_key(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "missing" },
        { name: "present", profile: { bio: "associated" } },
    ])
    .exec(&mut db)
    .await?;
    toasty::create!(Profile {
        bio: "unassociated"
    })
    .exec(&mut db)
    .await?;

    let users = User::filter(User::fields().profile().in_query(Profile::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(users, [{ name: "missing" }]);

    let users = User::filter(User::fields().profile().in_query(Profile::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(users, [{ name: "present" }]);

    Ok(())
}

#[driver_test(
    requires(scan),
    scenario(crate::scenarios::has_one_optional_belongs_to::id_uuid)
)]
pub async fn belongs_to_membership(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User {
        name: "present",
        profile: { bio: "associated" },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Profile {
        bio: "unassociated"
    })
    .exec(&mut db)
    .await?;

    let profiles = Profile::filter(Profile::fields().user().in_query(User::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "unassociated" }]);

    let profiles = Profile::filter(Profile::fields().user().in_query(User::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "associated" }]);

    let profiles = Profile::filter(
        Profile::fields()
            .user()
            .profile()
            .in_query(Profile::all())
            .not(),
    )
    .exec(&mut db)
    .await?;
    assert_struct!(profiles, [{ bio: "unassociated" }]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn membership_with_composite_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct Source {
        id: String,
        revision: i64,

        #[has_one]
        document: Option<Document>,
    }

    #[derive(Debug, toasty::Model)]
    #[unique(source_id, source_revision)]
    struct Document {
        #[key]
        id: String,

        source_id: Option<String>,
        source_revision: Option<i64>,

        #[belongs_to(key = [source_id, source_revision], references = [id, revision])]
        source: toasty::Deferred<Option<Source>>,
    }

    let mut db = t.setup_db(models!(Source, Document)).await;
    toasty::create!(Source::[
        { id: "source", revision: 1 },
        { id: "source", revision: 2, document: { id: "document" } },
    ])
    .exec(&mut db)
    .await?;
    toasty::create!(Document { id: "unassociated" })
        .exec(&mut db)
        .await?;

    let sources = Source::filter(Source::fields().document().in_query(Document::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ revision: 1 }]);

    let sources = Source::filter(Source::fields().document().in_query(Document::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ revision: 2 }]);

    let documents = Document::filter(Document::fields().source().in_query(Source::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(documents, [{ id: "unassociated" }]);

    Ok(())
}
