use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        title: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Post)).await
    }
}
