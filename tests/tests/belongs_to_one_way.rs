use tests::*;

use toasty::stmt::Id;

async fn crud_user_optional_profile_one_direction(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        profile_id: Option<Id<Profile>>,

        #[belongs_to(key = profile_id, references = id)]
        profile: Option<Profile>,
    }

    #[derive(Debug)]
    #[toasty::model]
    struct Profile {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = s.setup(models!(User, Profile)).await;

    // Create a user
    let user = User::create()
        .profile(Profile::create())
        .exec(&db)
        .await
        .unwrap();

    assert!(user.profile_id.is_some());
}

tests!(crud_user_optional_profile_one_direction,);
