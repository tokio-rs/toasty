use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn crud_user_optional_profile_one_direction(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        profile_id: Option<ID>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a user
    let user = User::create().profile(Profile::create()).exec(&db).await?;

    assert!(user.profile_id.is_some());
    Ok(())
}
