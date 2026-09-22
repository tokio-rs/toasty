use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[has_one]
        profile: toasty::Deferred<Option<Profile>>,

        #[has_one]
        settings: toasty::Deferred<Option<Settings>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: uuid::Uuid,

        bio: String,

        #[unique]
        user_id: Option<uuid::Uuid>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Settings {
        #[key]
        #[auto]
        id: uuid::Uuid,

        theme: String,

        #[unique]
        user_id: Option<uuid::Uuid>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Profile, Settings)).await
    }
}
