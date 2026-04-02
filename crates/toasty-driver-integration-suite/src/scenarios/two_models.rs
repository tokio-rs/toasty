use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[index]
        title: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Post)).await
    }
}
