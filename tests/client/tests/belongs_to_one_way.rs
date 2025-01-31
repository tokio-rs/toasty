use tests_client::*;

async fn crud_user_optional_profile_one_direction(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            #[index]
            profile_id: Option<Id<Profile>>,

            #[relation(key = profile_id, references = id)]
            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user
    let user = db::User::create()
        .profile(db::Profile::create())
        .exec(&db)
        .await
        .unwrap();

    assert!(!user.profile_id.is_none());
}

tests!(crud_user_optional_profile_one_direction,);
