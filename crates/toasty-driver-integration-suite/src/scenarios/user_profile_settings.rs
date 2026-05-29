use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

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
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Settings {
        #[key]
        #[auto]
        id: ID,

        theme: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Profile, Settings)).await
    }
}
