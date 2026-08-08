use crate::prelude::*;

#[driver_test]
pub async fn crud_user_optional_profile_one_direction(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        profile_id: Option<uuid::Uuid>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::Deferred<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }

    let mut db = test.setup_db(models!(User, Profile)).await;

    // Create a user
    let user = User::create()
        .profile(Profile::create())
        .exec(&mut db)
        .await?;

    assert!(user.profile_id.is_some());
    Ok(())
}
