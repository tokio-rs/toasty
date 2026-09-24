use crate::prelude::*;

#[driver_test(requires(scan))]
pub async fn variant_presence(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }

    #[derive(Debug, toasty::Embed)]
    enum State {
        Selected {
            user_id: Option<uuid::Uuid>,
            #[belongs_to(key = user_id, references = id)]
            user: toasty::Deferred<Option<User>>,
        },
        Other,
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        state: State,
    }

    let mut db = t.setup_db(models!(User, Item)).await;
    let user = toasty::create!(User {}).exec(&mut db).await?;
    let mut selected = Vec::new();
    for user_id in [None, Some(user.id)] {
        selected.push(
            toasty::create!(Item {
                state: State::Selected {
                    user_id,
                    user: toasty::Deferred::default(),
                }
            })
            .exec(&mut db)
            .await?,
        );
    }
    let other = toasty::create!(Item {
        state: State::Other
    })
    .exec(&mut db)
    .await?;

    let relation = || Item::fields().state().selected().user();
    for (predicate, expected) in [
        (relation().is_none(), vec![selected[0].id]),
        (relation().is_some(), vec![selected[1].id]),
        (relation().is_some().not(), vec![selected[0].id, other.id]),
        (relation().is_none().not(), vec![selected[1].id, other.id]),
    ] {
        let found = Item::filter(predicate).exec(&mut db).await?;
        assert_eq_unordered!(found.iter().map(|item| item.id), &expected);
    }
    Ok(())
}

#[driver_test(id(ID), requires(scan))]
pub async fn has_one_presence(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Source {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        document: toasty::Deferred<Option<Document>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        source_id: ID,

        #[belongs_to(key = source_id, references = id)]
        source: toasty::Deferred<Source>,
    }

    let mut db = t.setup_db(models!(Source, Document)).await;
    let missing = toasty::create!(Source {}).exec(&mut db).await?;

    let sources = Source::filter(Source::fields().document().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ id: == missing.id }]);
    assert!(
        Source::filter(Source::fields().document().is_some())
            .exec(&mut db)
            .await?
            .is_empty()
    );

    let present = toasty::create!(Source { document: {} })
        .exec(&mut db)
        .await?;

    let sources = Source::filter(Source::fields().document().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ id: == missing.id }]);

    let sources = Source::filter(Source::fields().document().is_some())
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ id: == present.id }]);

    let sources = Source::filter(
        Source::fields()
            .document()
            .is_none()
            .and(Source::fields().id().eq(present.id)),
    )
    .exec(&mut db)
    .await?;
    assert!(sources.is_empty());

    Ok(())
}

#[driver_test(
    requires(scan),
    scenario(crate::scenarios::has_one_optional_belongs_to::id_uuid)
)]
pub async fn belongs_to_presence(t: &mut Test) -> Result<()> {
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

    let profiles = Profile::filter(Profile::fields().user().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "unassociated" }]);

    let profiles = Profile::filter(Profile::fields().user().is_some())
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "associated" }]);

    let profiles = Profile::filter(Profile::fields().user().profile().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "unassociated" }]);

    Ok(())
}
