use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(toasty::models!(User)).await
    }
}
