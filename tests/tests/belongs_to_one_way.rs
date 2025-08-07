use tests::{models, tests, DbTest, Setup};
use toasty::stmt::Id;

async fn crud_user_optional_profile_one_direction(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        profile_id: Option<Id<Profile>>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a user
    let user = User::create()
        .profile(Profile::create())
        .exec(&db)
        .await
        .unwrap();

    assert!(user.profile_id.is_some());
}

tests!(crud_user_optional_profile_one_direction,);
